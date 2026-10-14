import React, { useMemo } from "react";

interface SyntaxHighlighterProps {
  code: string;
  extension: string;
}

interface Token {
  text: string;
  type: "comment" | "string" | "keyword" | "type" | "number" | "boolean" | "function" | "plain";
}

const RUST_KEYWORDS = new Set([
  "fn", "pub", "struct", "enum", "impl", "let", "mut", "match", "if", "else",
  "while", "for", "in", "return", "use", "mod", "crate", "self", "Self", "super",
  "async", "await", "const", "static", "where", "type", "trait", "unsafe", "ref", "move"
]);

const JS_KEYWORDS = new Set([
  "function", "const", "let", "var", "return", "if", "else", "for", "while",
  "import", "export", "from", "as", "class", "extends", "interface", "type",
  "async", "await", "new", "this", "typeof", "instanceof", "switch", "case",
  "default", "break", "continue", "try", "catch", "finally", "throw", "yield"
]);

const PY_KEYWORDS = new Set([
  "def", "class", "import", "from", "as", "return", "if", "elif", "else",
  "for", "while", "in", "try", "except", "finally", "raise", "with", "pass",
  "lambda", "yield", "is", "not", "and", "or", "global", "nonlocal", "assert"
]);

const SH_KEYWORDS = new Set([
  "if", "then", "fi", "elif", "else", "for", "do", "done", "case", "esac",
  "while", "until", "function", "return", "exit", "export", "source", "echo", "cd"
]);

const SQL_KEYWORDS = new Set([
  "select", "from", "where", "insert", "into", "values", "update", "set",
  "delete", "join", "left", "right", "inner", "outer", "on", "group", "by",
  "order", "limit", "having", "create", "table", "index", "drop", "alter", "as"
]);

const PRIMITIVE_TYPES = new Set([
  "i8", "i16", "i32", "i64", "i128", "isize",
  "u8", "u16", "u32", "u64", "u128", "usize",
  "f32", "f64", "bool", "char", "str", "String", "Vec", "Option", "Result",
  "int", "float", "list", "dict", "tuple", "set",
  "boolean", "number", "string", "any", "void", "never", "unknown", "Promise"
]);

const BOOLEANS_NULLS = new Set([
  "true", "false", "null", "undefined", "None", "True", "False", "Some", "Ok", "Err", "nil"
]);

function tokenizeLine(line: string, ext: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;
  const len = line.length;

  const isRust = ext === "rs";
  const isPython = ext === "py";
  const isJS = ["js", "jsx", "ts", "tsx"].includes(ext);
  const isShell = ["sh", "bash", "zsh"].includes(ext);
  const isSQL = ext === "sql";

  while (i < len) {
    const char = line[i];

    // 1. Comments
    if (
      (char === "/" && line[i + 1] === "/") ||
      (char === "#" && (isPython || isShell || ext === "yaml" || ext === "yml" || ext === "toml")) ||
      (char === "-" && line[i + 1] === "-" && isSQL)
    ) {
      tokens.push({ text: line.slice(i), type: "comment" });
      break;
    }

    // 2. Strings: ", ', or `
    if (char === '"' || char === "'" || char === "`") {
      const quote = char;
      let end = i + 1;
      while (end < len) {
        if (line[end] === "\\" && end + 1 < len) {
          end += 2;
          continue;
        }
        if (line[end] === quote) {
          end++;
          break;
        }
        end++;
      }
      tokens.push({ text: line.slice(i, end), type: "string" });
      i = end;
      continue;
    }

    // 3. Numbers
    if (/[0-9]/.test(char) && (i === 0 || !/[a-zA-Z0-9_]/.test(line[i - 1]))) {
      let end = i + 1;
      while (end < len && /[0-9a-fA-FxX._]/.test(line[end])) {
        end++;
      }
      tokens.push({ text: line.slice(i, end), type: "number" });
      i = end;
      continue;
    }

    // 4. Identifiers & Keywords
    if (/[a-zA-Z_]/.test(char)) {
      let end = i + 1;
      while (end < len && /[a-zA-Z0-9_]/.test(line[end])) {
        end++;
      }
      const word = line.slice(i, end);
      const lowerWord = word.toLowerCase();

      // Check booleans / constants
      if (BOOLEANS_NULLS.has(word)) {
        tokens.push({ text: word, type: "boolean" });
      }
      // Check keywords
      else if (
        (isRust && RUST_KEYWORDS.has(word)) ||
        (isJS && JS_KEYWORDS.has(word)) ||
        (isPython && PY_KEYWORDS.has(word)) ||
        (isShell && SH_KEYWORDS.has(word)) ||
        (isSQL && SQL_KEYWORDS.has(lowerWord)) ||
        (!isRust && !isJS && !isPython && !isShell && !isSQL && (RUST_KEYWORDS.has(word) || JS_KEYWORDS.has(word)))
      ) {
        tokens.push({ text: word, type: "keyword" });
      }
      // Check types (built-in or PascalCase)
      else if (PRIMITIVE_TYPES.has(word) || /^[A-Z][a-zA-Z0-9_]*$/.test(word)) {
        tokens.push({ text: word, type: "type" });
      }
      // Check functions (lookahead for '(')
      else {
        let peek = end;
        while (peek < len && (line[peek] === " " || line[peek] === "\t")) {
          peek++;
        }
        if (peek < len && line[peek] === "(") {
          tokens.push({ text: word, type: "function" });
        } else {
          tokens.push({ text: word, type: "plain" });
        }
      }

      i = end;
      continue;
    }

    // 5. Symbols / whitespace
    tokens.push({ text: char, type: "plain" });
    i++;
  }

  return tokens;
}

export const SyntaxHighlighter: React.FC<SyntaxHighlighterProps> = ({ code, extension }) => {
  const ext = extension.toLowerCase();

  const lines = useMemo(() => {
    return code.split("\n");
  }, [code]);

  return (
    <div className="syntax-code-container">
      <div className="syntax-code-table">
        {lines.map((line, idx) => {
          const tokens = tokenizeLine(line, ext);
          return (
            <div key={idx} className="syntax-line-row">
              <span className="syntax-line-num" aria-hidden="true">
                {idx + 1}
              </span>
              <span className="syntax-line-content">
                {tokens.length === 0 ? "\u00A0" : (
                  tokens.map((tok, tIdx) => (
                    <span
                      key={tIdx}
                      className={tok.type !== "plain" ? `syn-${tok.type}` : undefined}
                    >
                      {tok.text}
                    </span>
                  ))
                )}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
};
