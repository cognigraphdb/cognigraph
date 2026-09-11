import { type Diagnostic, forceLinting, linter, lintGutter, lintKeymap } from "@codemirror/lint";
import { EditorState, Prec, StateEffect } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { basicSetup } from "codemirror";
import { useEffect, useRef } from "react";
import { cgqlLanguage } from "../lib/cgql-language.ts";
import "../styles/cgql-editor.css";

interface CgqlEditorProps {
  value: string;
  validationRevision: string;
  validate: (query: string) => Promise<readonly Diagnostic[]>;
  onChange: (value: string) => void;
  onRun: () => void;
}

const validationRevisionEffect = StateEffect.define<string>();

export function CgqlEditor({
  value,
  validationRevision,
  validate,
  onChange,
  onRun,
}: CgqlEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const validateRef = useRef(validate);
  const onChangeRef = useRef(onChange);
  const onRunRef = useRef(onRun);
  const initialValueRef = useRef(value);
  validateRef.current = validate;
  onChangeRef.current = onChange;
  onRunRef.current = onRun;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const runQueryShortcut = () => {
      onRunRef.current();
      return true;
    };
    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: initialValueRef.current,
        extensions: [
          basicSetup,
          cgqlLanguage,
          lintGutter(),
          linter((editor) => validateRef.current(editor.state.doc.toString()), {
            delay: 550,
            needsRefresh: (update) =>
              update.transactions.some((transaction) =>
                transaction.effects.some((effect) => effect.is(validationRevisionEffect)),
              ),
          }),
          Prec.highest(
            keymap.of([
              {
                key: "Mod-Enter",
                run: runQueryShortcut,
              },
              {
                key: "Ctrl-Enter",
                run: runQueryShortcut,
              },
              ...lintKeymap,
            ]),
          ),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({
            "aria-label": "CGQL query",
            autocapitalize: "off",
            spellcheck: "false",
          }),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) onChangeRef.current(update.state.doc.toString());
          }),
          editorTheme,
        ],
      }),
    });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, []);

  useEffect(() => {
    const view = viewRef.current;
    if (!view || value === view.state.doc.toString()) return;
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } });
  }, [value]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({ effects: validationRevisionEffect.of(validationRevision) });
    forceLinting(view);
  }, [validationRevision]);

  return <div className="cgql-editor" ref={hostRef} />;
}

const editorTheme = EditorView.theme({
  "&": {
    height: "100%",
    backgroundColor: "#fbfcfb",
    color: "#183f54",
    fontSize: "13px",
  },
  ".cm-content": {
    padding: "14px 0",
    fontFamily: '"SFMono-Regular", Consolas, monospace',
    lineHeight: "1.7",
  },
  ".cm-gutters": {
    borderRight: "1px solid #e2e7e5",
    backgroundColor: "#f5f7f6",
    color: "#8a9693",
  },
  ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "#eef7f6" },
  ".cm-cursor": { borderLeftColor: "#087f7c" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
    backgroundColor: "#cfe8e5 !important",
  },
  "&.cm-focused": { outline: "none" },
});
