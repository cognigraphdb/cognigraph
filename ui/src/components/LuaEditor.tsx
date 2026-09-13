import { StreamLanguage } from "@codemirror/language";
import { lua } from "@codemirror/legacy-modes/mode/lua";
import { EditorState, Prec } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { basicSetup } from "codemirror";
import { useEffect, useRef } from "react";
import "../styles/cgql-editor.css";

interface LuaEditorProps {
  value: string;
  onChange: (value: string) => void;
  onRun: () => void;
}

/// CodeMirror editor for the Lua console — the CGQL editor's shell with
/// Lua highlighting and no lint pipeline (scripts fail server-side with
/// the engine's own line-numbered message).
export function LuaEditor({ value, onChange, onRun }: LuaEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  const onRunRef = useRef(onRun);
  const initialValueRef = useRef(value);
  onChangeRef.current = onChange;
  onRunRef.current = onRun;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const runShortcut = () => {
      onRunRef.current();
      return true;
    };
    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: initialValueRef.current,
        extensions: [
          basicSetup,
          StreamLanguage.define(lua),
          Prec.highest(
            keymap.of([
              { key: "Mod-Enter", run: runShortcut },
              { key: "Ctrl-Enter", run: runShortcut },
            ]),
          ),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({
            "aria-label": "Lua script",
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
