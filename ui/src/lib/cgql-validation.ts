import type { Diagnostic } from "@codemirror/lint";

export interface PreparedCgqlValidation {
  query: string;
  lineOffset: number;
}

export function prepareCgqlValidation(query: string): PreparedCgqlValidation {
  const leadingTrivia = query.match(/^(?:(?:\s+)|(?:\/\/[^\n]*(?:\n|$)))*/)?.[0] ?? "";
  const body = query.slice(leadingTrivia.length);
  const analyze = /^EXPLAIN(\s+)(ANALYZE)\b/i.exec(body);
  if (analyze) {
    const whitespace = analyze[1] ?? "";
    const analyzeKeyword = analyze[2] ?? "ANALYZE";
    const analyzeStart = leadingTrivia.length + "EXPLAIN".length + whitespace.length;
    return {
      query: `${query.slice(0, analyzeStart)}${" ".repeat(analyzeKeyword.length)}${query.slice(
        analyzeStart + analyzeKeyword.length,
      )}`,
      lineOffset: 0,
    };
  }
  if (/^EXPLAIN\b/i.test(body)) return { query, lineOffset: 0 };
  return { query: `EXPLAIN\n${query}`, lineOffset: 1 };
}

export function diagnosticFromCgqlError(
  query: string,
  errorMessage: string,
  lineOffset: number,
): Diagnostic {
  const message = errorMessage.replace(/^Query execution error:\s*/i, "");
  const parseLocation = /parse error at line (\d+), column (\d+):\s*(.+)$/i.exec(message);
  if (parseLocation) {
    const line = Number(parseLocation[1]) - lineOffset;
    const column = Number(parseLocation[2]);
    const from = offsetAt(query, line, column);
    return diagnostic(query, from, parseLocation[3] ?? message);
  }

  const missingBind = /missing bind variables?:\s*([A-Za-z_][A-Za-z0-9_]*)/i.exec(message);
  if (missingBind) return diagnosticAtTerm(query, `@${missingBind[1] ?? ""}`, message);

  const namedTerm = /(?:identifier|function) `([^`]+)`/i.exec(message);
  if (namedTerm) return diagnosticAtTerm(query, namedTerm[1] ?? "", message);

  return diagnostic(query, 0, message);
}

export function emptyQueryDiagnostic(): Diagnostic {
  return {
    from: 0,
    to: 0,
    severity: "error",
    source: "CGQL",
    message: "Enter a CGQL query",
  };
}

export function unavailableDiagnostic(query: string, message: string): Diagnostic {
  return {
    from: 0,
    to: Math.min(query.length, 1),
    severity: "warning",
    source: "CGQL server",
    message: `Validation unavailable: ${message}`,
  };
}

export function parseBindVariables(
  source: string,
): { value: Record<string, unknown> } | { error: string } {
  try {
    const value: unknown = JSON.parse(source);
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
      return { error: "Bind variables must be a JSON object" };
    }
    return { value: value as Record<string, unknown> };
  } catch {
    return { error: "Bind variables must be valid JSON" };
  }
}

export function missingBindVariableDiagnostics(
  query: string,
  bindVariables: Record<string, unknown>,
): Diagnostic[] {
  const diagnostics: Diagnostic[] = [];
  const reported = new Set<string>();
  let inString = false;
  let escaped = false;

  for (let index = 0; index < query.length; index += 1) {
    const character = query[index];
    if (inString) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') inString = false;
      continue;
    }
    if (character === '"') {
      inString = true;
      continue;
    }
    if (character === "/" && query[index + 1] === "/") {
      while (index < query.length && query[index] !== "\n") index += 1;
      continue;
    }
    if (character !== "@") continue;
    const match = /^[A-Za-z_][A-Za-z0-9_]*/.exec(query.slice(index + 1));
    const name = match?.[0];
    if (!name || name in bindVariables || reported.has(name)) continue;
    diagnostics.push(diagnostic(query, index, `Missing bind variable: ${name}`, name.length + 1));
    reported.add(name);
    index += name.length;
  }
  return diagnostics;
}

function offsetAt(query: string, line: number, column: number) {
  if (line < 1) return 0;
  const lines = query.split("\n");
  const lineIndex = Math.min(line - 1, lines.length - 1);
  let offset = 0;
  for (let index = 0; index < lineIndex; index += 1) offset += (lines[index]?.length ?? 0) + 1;
  return Math.min(offset + Math.max(column - 1, 0), query.length);
}

function diagnosticAtTerm(query: string, term: string, message: string) {
  const from = query.indexOf(term);
  return diagnostic(query, from >= 0 ? from : 0, message, from >= 0 ? term.length : undefined);
}

function diagnostic(query: string, from: number, message: string, length?: number): Diagnostic {
  const safeFrom = Math.min(Math.max(from, 0), query.length);
  const tokenEnd = query.slice(safeFrom).search(/[\s,()[\]{}]/);
  const inferredLength = tokenEnd > 0 ? tokenEnd : 1;
  return {
    from: safeFrom,
    to: Math.min(query.length, safeFrom + (length ?? inferredLength)),
    severity: "error",
    source: "CGQL",
    message,
  };
}
