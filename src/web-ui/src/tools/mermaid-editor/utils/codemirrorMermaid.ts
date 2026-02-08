/**
 * CodeMirror Mermaid syntax highlighting.
 */

import { LanguageSupport, StreamLanguage } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';

// Mermaid tokenization rules.
const mermaidLanguage = StreamLanguage.define({
  name: 'mermaid',
  startState() {
    return {
      inComment: false,
      inString: false,
      stringDelim: null
    };
  },
  
  token(stream, state) {
    // Order matters: handle comments and strings first.
    if (stream.match(/%%.*$/)) {
      return 'comment';
    }
    
    if (!state.inString && (stream.match('"') || stream.match("'"))) {
      state.inString = true;
      state.stringDelim = stream.current();
      return 'string';
    }
    
    if (state.inString) {
      if (stream.match(state.stringDelim!)) {
        state.inString = false;
        state.stringDelim = null;
        return 'string';
      }
      stream.next();
      return 'string';
    }
    
    if (stream.match(/^(graph|flowchart|sequenceDiagram|classDiagram|stateDiagram|erDiagram|journey|gantt|pie|gitgraph|mindmap|timeline|quadrantChart)\b/)) {
      return 'keyword';
    }

    if (stream.match(/^(TD|TB|BT|RL|LR)\b/)) {
      return 'keyword';
    }

    if (stream.match(/^(participant|actor|note|loop|alt|else|opt|par|and|rect|activate|deactivate|state|class|relationship|title)\b/)) {
      return 'keyword';
    }

    if (stream.match(/-->|->|---|--\|[^|]*\||==>/)) {
      return 'operator';
    }

    if (stream.match(/[\[\](){}><]/)) {
      return 'bracket';
    }

    if (stream.match(/^\d+(\.\d+)?/)) {
      return 'number';
    }

    if (stream.match(/^[A-Za-z_\u4e00-\u9fff][A-Za-z0-9_\u4e00-\u9fff]*/)) {
      return 'variableName';
    }

    if (stream.match(/[+\-*/%=<>!&|]/)) {
      return 'operator';
    }

    if (stream.match(/[{}[\];(),.:]/)) {
      return 'punctuation';
    }
    
    stream.next();
    return null;
  }
});

// Highlight styles.
export const mermaidHighlightStyle = [
  { tag: t.comment, color: '#6a9955', fontStyle: 'italic' },
  { tag: t.keyword, color: '#569cd6', fontWeight: 'bold' },
  { tag: t.string, color: '#ce9178' },
  { tag: t.number, color: '#b5cea8' },
  { tag: t.variableName, color: '#9cdcfe' },
  { tag: t.operator, color: '#d4d4d4', fontWeight: 'bold' },
  { tag: t.bracket, color: '#ffd700' },
  { tag: t.punctuation, color: '#d4d4d4' }
];

// Language support export.
export const mermaid = () => new LanguageSupport(mermaidLanguage);
