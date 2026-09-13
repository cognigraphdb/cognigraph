import {
  HighlightStyle,
  LanguageSupport,
  StreamLanguage,
  type StreamParser,
  type StringStream,
  syntaxHighlighting,
} from "@codemirror/language";
import { tags } from "@lezer/highlight";

interface CgqlStreamState {
  inString: boolean;
  escaped: boolean;
  afterDot: boolean;
}

const keywords = new Set([
  "AGGREGATE",
  "ANALYZE",
  "AND",
  "ANY",
  "ASC",
  "COLLATE",
  "COLLECT",
  "COUNT",
  "DESC",
  "DISTINCT",
  "EXPLAIN",
  "FILTER",
  "FOR",
  "IN",
  "INBOUND",
  "INSERT",
  "INTO",
  "LET",
  "LIMIT",
  "NOT",
  "OR",
  "OUTBOUND",
  "REMOVE",
  "REPLACE",
  "RETURN",
  "SORT",
  "UPDATE",
  "UPSERT",
  "VECTOR_SEARCH",
  "WITH",
]);

const functions = new Set([
  "ABS",
  "AVG",
  "CEIL",
  "CONCAT",
  "CONTAINS",
  "COSINE_SIMILARITY",
  "DATE_DAY",
  "DATE_DIFF",
  "DATE_HOUR",
  "DATE_MINUTE",
  "DATE_MONTH",
  "DATE_SECOND",
  "DATE_TIMESTAMP",
  "DATE_YEAR",
  "FIRST",
  "FLOOR",
  "HAS",
  "LAST",
  "LENGTH",
  "LOWER",
  "MAX",
  "MIN",
  "NOW",
  "ROUND",
  "SPLIT",
  "STARTS_WITH",
  "SUBSTRING",
  "SUM",
  "TRIM",
  "TYPENAME",
  "UNIQUE",
  "UPPER",
]);

const parser: StreamParser<CgqlStreamState> = {
  name: "cgql",
  startState: () => ({ inString: false, escaped: false, afterDot: false }),
  token(stream, state) {
    if (state.inString) return readString(stream, state);
    if (stream.eatSpace()) return null;
    if (stream.match("//")) {
      stream.skipToEnd();
      return "comment";
    }
    if (stream.peek() === '"') {
      stream.next();
      state.inString = true;
      return readString(stream, state);
    }
    if (stream.match(/^@[A-Za-z_][A-Za-z0-9_]*/)) return "variableName.special";
    if (stream.match(/^-?\d+(?:\.\d+)?/)) return "number";
    if (stream.match(/^(?:==|!=|<=|>=|\.\.|[+*/<>=-])/)) return "operator";
    if (stream.match(".")) {
      state.afterDot = true;
      return "punctuation";
    }
    if (stream.match(/^[{}[\](),:]/)) return "punctuation";
    if (stream.match(/^[A-Za-z_][A-Za-z0-9_]*(?=\s*:)/)) return "propertyName";
    if (stream.match(/^[A-Za-z_][A-Za-z0-9_]*/)) {
      const value = stream.current().toUpperCase();
      if (value === "TRUE" || value === "FALSE") return "bool";
      if (value === "NULL") return "null";
      if (keywords.has(value)) return "keyword";
      if (functions.has(value)) return "function(variableName)";
      if (state.afterDot) {
        state.afterDot = false;
        return "propertyName";
      }
      return "variableName";
    }
    state.afterDot = false;
    stream.next();
    return null;
  },
  languageData: {
    closeBrackets: { brackets: ["(", "[", "{", '"'] },
    commentTokens: { line: "//" },
  },
};

function readString(stream: StringStream, state: CgqlStreamState) {
  while (!stream.eol()) {
    const character = stream.next();
    if (state.escaped) {
      state.escaped = false;
    } else if (character === "\\") {
      state.escaped = true;
    } else if (character === '"') {
      state.inString = false;
      break;
    }
  }
  return "string";
}

const highlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: "#0b6478", fontWeight: "600" },
  { tag: tags.function(tags.variableName), color: "#7654a8" },
  { tag: tags.variableName, color: "#183f54" },
  { tag: tags.special(tags.variableName), color: "#a15c00" },
  { tag: tags.propertyName, color: "#426b7c" },
  { tag: tags.string, color: "#b13c52" },
  { tag: tags.number, color: "#188249" },
  { tag: [tags.bool, tags.null], color: "#7654a8" },
  { tag: tags.comment, color: "#7a8784", fontStyle: "italic" },
  { tag: tags.operator, color: "#53605e" },
  { tag: tags.punctuation, color: "#687572" },
]);

export const cgqlLanguage = new LanguageSupport(StreamLanguage.define(parser), [
  syntaxHighlighting(highlightStyle),
]);
