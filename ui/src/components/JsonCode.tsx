interface JsonCodeProps {
  value: string;
}

const tokenPattern =
  /("(?:\\.|[^"\\])*")(?=\s*:)|("(?:\\.|[^"\\])*")|\b(true|false|null)\b|(-?\d+(?:\.\d+)?)/g;

function highlightLine(line: string) {
  const parts = line.split(tokenPattern).filter((part) => part !== undefined && part !== "");
  let inKeyPosition = true;
  let offset = 0;

  return parts.map((part) => {
    const tokenOffset = line.indexOf(part, offset);
    offset = tokenOffset + part.length;
    let className = "json-punctuation";
    if (part.startsWith('"')) {
      className = inKeyPosition && line.includes(`${part}:`) ? "json-key" : "json-string";
      if (className === "json-key") inKeyPosition = false;
    } else if (/^-?\d/.test(part)) {
      className = "json-number";
    } else if (/^(true|false|null)$/.test(part)) {
      className = "json-literal";
    }
    return (
      <span className={className} key={`${tokenOffset}-${part}`}>
        {part}
      </span>
    );
  });
}

function linesWithOffsets(value: string) {
  let offset = 0;
  return value.split("\n").map((line) => {
    const item = { id: `${offset}-${line}`, line };
    offset += line.length + 1;
    return item;
  });
}

export function JsonCode({ value }: JsonCodeProps) {
  return (
    <ol className="json-code">
      {linesWithOffsets(value).map(({ id, line }) => (
        <li key={id}>
          <code>{highlightLine(line)}</code>
        </li>
      ))}
    </ol>
  );
}
