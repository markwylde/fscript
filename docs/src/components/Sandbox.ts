import * as monaco from 'monaco-editor/esm/vs/editor/editor.api';
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import initWasm, {
  check as wasmCheck,
  run as wasmRun,
} from '../generated/fscript-wasm/fscript_wasm.js';

const LANGUAGE_ID = 'fscript';
const MODEL_URI = monaco.Uri.parse('inmemory://model/sandbox.fs');
const HASH_PREFIX = '#code=';

type SandboxDiagnostic = {
  kind: string;
  title: string;
  message: string;
  line?: number;
  column?: number;
  width?: number;
  location?: string;
  label?: string;
};

type SandboxResult = {
  ok: boolean;
  token_count?: number;
  output?: string;
  pretty_error?: string;
  diagnostic?: SandboxDiagnostic;
};

declare global {
  interface Window {
    MonacoEnvironment?: {
      getWorker: () => Worker;
    };
  }
}

if (!window.MonacoEnvironment) {
  window.MonacoEnvironment = {
    getWorker: () => new editorWorker(),
  };
}

let registered = false;
let wasmReady: Promise<void> | undefined;

export function mountSandboxes() {
  const roots = document.querySelectorAll<HTMLElement>('[data-fscript-sandbox]');

  for (const root of roots) {
    if (root.dataset.mounted === 'true') {
      continue;
    }

    root.dataset.mounted = 'true';
    void mountSandbox(root);
  }
}

async function mountSandbox(root: HTMLElement) {
  registerLanguage();

  const codeElement = root.querySelector<HTMLElement>('[data-role="initial-code"]');
  const initialCode = (codeElement?.textContent || readCodeFromHash()).trim();
  const editorElement = root.querySelector<HTMLElement>('[data-role="editor"]');
  const outputElement = root.querySelector<HTMLElement>('[data-role="output"]');
  const statusElement = root.querySelector<HTMLElement>('[data-role="status"]');
  const runButton = root.querySelector<HTMLButtonElement>('[data-role="run"]');
  const shareButton = root.querySelector<HTMLButtonElement>('[data-role="share"]');

  if (!editorElement || !outputElement || !statusElement || !runButton || !shareButton) {
    return;
  }

  const model = monaco.editor.createModel(initialCode, LANGUAGE_ID, MODEL_URI);
  const editor = monaco.editor.create(editorElement, {
    model,
    automaticLayout: true,
    minimap: { enabled: false },
    fontSize: 14,
    lineHeight: 24,
    fontFamily: "var(--monaco-monospace-font, monospace)",
    roundedSelection: false,
    scrollBeyondLastLine: false,
    wordWrap: 'off',
    wrappingIndent: 'indent',
    padding: { top: 16, bottom: 16 },
    theme: 'fscript-night',
    fixedOverflowWidgets: true,
    fontLigatures: true,
    renderLineHighlight: 'all',
    cursorBlinking: 'smooth',
    cursorSmoothCaretAnimation: 'on',
    scrollbar: {
      horizontal: 'auto',
      vertical: 'auto',
    },
  });

  // Comprehensive layout force
  const doLayout = () => {
    editor.layout();
  };

  // Immediate and delayed layout to catch container sizing changes
  doLayout();
  setTimeout(doLayout, 50);
  setTimeout(doLayout, 250);
  setTimeout(doLayout, 1000);

  // Use ResizeObserver for more robust layout updates
  if ('ResizeObserver' in window) {
    const ro = new ResizeObserver(() => {
      requestAnimationFrame(doLayout);
    });
    ro.observe(editorElement);
  }

  window.addEventListener('resize', doLayout);

  setStatus(statusElement, 'Loading compiler...');
  outputElement.textContent = 'Compiling WebAssembly module...';

  try {
    await ensureWasm();
    const checkResult = (await wasmCheck(model.getValue())) as SandboxResult;
    syncDiagnostics(model, checkResult.diagnostic);
    setReadyState(statusElement, checkResult);
    outputElement.textContent = 'Press Run to execute the current program.';
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    setStatus(statusElement, 'WASM failed to load');
    outputElement.textContent = message;
    return;
  }

  let checkTimer: number | undefined;

  model.onDidChangeContent(() => {
    window.clearTimeout(checkTimer);
    const code = model.getValue();
    persistCode(code);
    setStatus(statusElement, 'Checking...');

    checkTimer = window.setTimeout(async () => {
      const result = (await wasmCheck(code)) as SandboxResult;
      syncDiagnostics(model, result.diagnostic);
      setReadyState(statusElement, result);
    }, 220);
  });

  runButton.addEventListener('click', async () => {
    setStatus(statusElement, 'Running...');
    outputElement.innerHTML = 'Executing...';

    const result = (await wasmRun(model.getValue())) as SandboxResult;
    syncDiagnostics(model, result.diagnostic);

    if (result.ok) {
      outputElement.innerHTML = ansiToHtml(result.output ?? '(no output)');
      setStatus(statusElement, 'Execution finished');
      return;
    }

    outputElement.innerHTML = ansiToHtml(
      result.pretty_error ?? result.diagnostic?.message ?? 'Execution failed.',
    );
    setReadyState(statusElement, result);
  });

  shareButton.addEventListener('click', async () => {
    const url = new URL(window.location.href);
    url.hash = `${HASH_PREFIX}${encodeURIComponent(model.getValue())}`;

    try {
      await navigator.clipboard.writeText(url.toString());
      setStatus(statusElement, 'Share link copied');
    } catch {
      setStatus(statusElement, 'Copy failed, URL updated');
    }

    history.replaceState(null, '', url);
  });
}

function ansiToHtml(text: string): string {
  const map: Record<string, string> = {
    '1': 'font-weight: bold',
    '30': 'color: #1e293b', // slate-900
    '31': 'color: #f43f5e', // rose-500
    '32': 'color: #10b981', // emerald-500
    '33': 'color: #f59e0b', // amber-500
    '34': 'color: #3b82f6', // blue-500
    '35': 'color: #a855f7', // purple-500
    '36': 'color: #06b6d4', // cyan-500
    '37': 'color: #f8fafc', // slate-50
    '90': 'color: #64748b', // slate-500
  };

  const escaped = text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');

  return escaped.replace(/\x1b\[([\d;]+)m/g, (_, codes) => {
    if (codes === '0') return '</span>';
    const styles = codes
      .split(';')
      .map((c: string) => map[c])
      .filter(Boolean);
    if (styles.length === 0) return '';
    return `<span style="${styles.join(';')}">`;
  });
}

function registerLanguage() {
  if (registered) {
    return;
  }

  registered = true;
  monaco.languages.register({ id: LANGUAGE_ID });
  monaco.languages.setMonarchTokensProvider(LANGUAGE_ID, {
    defaultToken: '',
    keywords: [
      'if',
      'else',
      'match',
      'try',
      'catch',
      'throw',
      'import',
      'from',
      'export',
      'yield',
      'defer',
      'return',
      'type',
      'as',
    ],
    typeKeywords: ['String', 'Boolean', 'Number', 'Array', 'Result', 'Task', 'Void', 'Never'],
    constants: ['true', 'false', 'null', 'undefined'],
    operators: [
      '|>',
      '=>',
      '...',
      '===',
      '!==',
      '==',
      '!=',
      '<=',
      '>=',
      '??',
      '&&',
      '||',
      '<',
      '>',
      '=',
      '+',
      '-',
      '*',
      '/',
      '%',
      '!',
      '.',
      ':',
    ],
    symbols: /[=><!~?:&|+\-*\/\^%\.]+/,
    tokenizer: {
      root: [
        [/\/\/.*$/, 'comment'],
        [/\/\*/, 'comment', '@comment'],
        [/'([^'\\]|\\.)*'?/, 'string'],
        [/"([^"\\]|\\.)*"?/, 'string'],
        [/\b\d+(\.\d+)?\b/, 'number'],
        [/[{}\[\]()]/, '@brackets'],
        [/[a-z_$][\w$]*/, {
          cases: {
            '@keywords': 'keyword',
            '@constants': 'constant.language',
            '@default': 'identifier',
          },
        }],
        [/[A-Z][\w$]*/, {
          cases: {
            '@typeKeywords': 'type.identifier',
            '@default': 'identifier',
          },
        }],
        [/@symbols/, {
          cases: {
            '@operators': 'operator',
            '@default': '',
          },
        }],
      ],
      comment: [
        [/[^/*]+/, 'comment'],
        [/\*\//, 'comment', '@pop'],
        [/./, 'comment'],
      ],
    },
  });

  monaco.editor.defineTheme('fscript-night', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '64748b', fontStyle: 'italic' },
      { token: 'string', foreground: 'fbbf24' },
      { token: 'number', foreground: '7dd3fc' },
      { token: 'keyword', foreground: '34d399', fontStyle: 'bold' },
      { token: 'operator', foreground: 'e2e8f0' },
      { token: 'type.identifier', foreground: 'fda4af' },
      { token: 'constant.language', foreground: 'c4b5fd' },
      { token: 'identifier', foreground: 'f8fafc' },
    ],
    colors: {
      'editor.background': '#020617',
      'editorLineNumber.foreground': '#475569',
      'editorLineNumber.activeForeground': '#cbd5e1',
      'editorCursor.foreground': '#34d399',
      'editor.selectionBackground': '#0f766e55',
      'editor.inactiveSelectionBackground': '#1e293b99',
      'editorIndentGuide.background1': '#1e293b',
      'editorIndentGuide.activeBackground1': '#475569',
    },
  });
}

function syncDiagnostics(model: monaco.editor.ITextModel, diagnostic?: SandboxDiagnostic) {
  if (!diagnostic?.line || !diagnostic?.column) {
    monaco.editor.setModelMarkers(model, LANGUAGE_ID, []);
    return;
  }

  monaco.editor.setModelMarkers(model, LANGUAGE_ID, [
    {
      message: diagnostic.message,
      severity: monaco.MarkerSeverity.Error,
      startLineNumber: diagnostic.line,
      startColumn: diagnostic.column,
      endLineNumber: diagnostic.line,
      endColumn: diagnostic.column + Math.max(diagnostic.width ?? 1, 1),
      source: diagnostic.kind,
    },
  ]);
}

function setReadyState(statusElement: HTMLElement, result: SandboxResult) {
  if (result.ok) {
    const suffix = typeof result.token_count === 'number' ? ` (${result.token_count} tokens)` : '';
    setStatus(statusElement, `Ready${suffix}`);
    return;
  }

  const location = result.diagnostic?.line ? ` line ${result.diagnostic.line}` : '';
  setStatus(statusElement, `${result.diagnostic?.title ?? 'Error'}${location}`);
}

function setStatus(element: HTMLElement, text: string) {
  element.textContent = text;
}

async function ensureWasm() {
  if (!wasmReady) {
    wasmReady = initWasm();
  }

  await wasmReady;
}

function readCodeFromHash() {
  if (!window.location.hash.startsWith(HASH_PREFIX)) {
    return '';
  }

  try {
    return decodeURIComponent(window.location.hash.slice(HASH_PREFIX.length));
  } catch {
    return '';
  }
}

function persistCode(code: string) {
  const url = new URL(window.location.href);
  url.hash = `${HASH_PREFIX}${encodeURIComponent(code)}`;
  history.replaceState(null, '', url);
}
