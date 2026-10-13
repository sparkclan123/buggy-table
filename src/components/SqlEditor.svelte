<script lang="ts">
  import { onMount } from 'svelte';
  import { EditorState } from '@codemirror/state';
  import { EditorView, keymap } from '@codemirror/view';
  import { basicSetup } from 'codemirror';
  import { sql, PostgreSQL } from '@codemirror/lang-sql';

  let {
    value,
    onchange,
    onrun,
    schema,
  }: {
    value: string;
    onchange: (value: string) => void;
    onrun: () => void;
    schema: Record<string, string[]>;
  } = $props();
  let element: HTMLDivElement;
  let view: EditorView | undefined;
  onMount(() => {
    view = new EditorView({
      parent: element,
      state: EditorState.create({
        doc: value,
        extensions: [
          basicSetup,
          sql({ dialect: PostgreSQL, schema }),
          keymap.of([
            {
              key: 'Mod-Enter',
              run: () => {
                onrun();
                return true;
              },
            },
          ]),
          EditorView.contentAttributes.of({
            'aria-label': 'SQL query editor',
            spellcheck: 'false',
          }),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) onchange(update.state.doc.toString());
          }),
          EditorView.theme({
            '&': { fontSize: '12px', minHeight: '120px', background: '#f8f9fb' },
            '.cm-content': {
              padding: '8px 0',
              fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
            },
            '.cm-gutters': {
              background: '#f8f9fb',
              border: 'none',
              color: '#a4aaa7',
              padding: '8px 8px 0 6px',
            },
            '&.cm-focused': { outline: 'none' },
            '.cm-activeLine': { background: '#eef0f3' },
            '.cm-activeLineGutter': { background: 'transparent' },
            '.cm-scroller': { maxHeight: '240px', overflow: 'auto' },
          }),
        ],
      }),
    });
    return () => view?.destroy();
  });
  $effect(() => {
    if (view && view.state.doc.toString() !== value)
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } });
  });
</script>

<div class="sql-editor" bind:this={element}></div>
