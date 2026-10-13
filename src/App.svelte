<script lang="ts">
  import {
    Database,
    Table2,
    Plus,
    Search,
    ChevronRight,
    ChevronDown,
    ShieldCheck,
    Activity,
    Settings2,
    Code2,
    Columns3,
    ListTree,
    RefreshCw,
    Filter as FilterIcon,
    Play,
    Square,
    LockKeyhole,
    ArrowUpRight,
    ChevronLeft,
    PlugZap,
    FlaskConical,
    Sparkles,
    X,
    PanelLeft,
    CircleHelp,
    Check,
    Unplug,
  } from '@lucide/svelte';
  import ThemeControl from './components/ThemeControl.svelte';
  import UpdateNotice from './components/UpdateNotice.svelte';
  let updating = $state(false);
  import { onMount } from 'svelte';
  import { guardWindowClose } from './lib/window-lifecycle';
  import { createWorkspace, changeCell, changedRows, type TableView as Tab } from './lib/workspace';
  import RedisWorkspace from './components/RedisWorkspace.svelte';
  import ConfirmClose from './components/ConfirmClose.svelte';
  import ConnectionDialog from './components/ConnectionDialog.svelte';
  import ResultGrid from './components/ResultGrid.svelte';
  import FilterBuilder from './components/FilterBuilder.svelte';
  import { api, errorMessage, desktopAvailable } from './lib/api';
  import { browseSample, sampleConnection, sampleIndexes, sampleRows } from './lib/sample';
  import type { ConnectionInfo, Overview, TableInfo } from './lib/types';

  type Page = 'explorer' | 'overview' | 'privacy' | 'assistant';
  let connection = $state<ConnectionInfo>(sampleConnection);
  const initial = createWorkspace(sampleConnection.tables[0]);
  initial.result = browseSample(sampleConnection.tables[0], [], 0);
  let openTables = $state([initial]);
  let activeId = $state(initial.id);
  let workspace = $derived(openTables.find((t) => t.id === activeId)!);
  let closeTarget = $state<string | null>(null);
  const dirtyCount = $derived(Object.keys(workspace.edits).length);
  const anyDirty = $derived(openTables.some((t) => Object.keys(t.edits).length > 0 || t.uncertain));
  let saving = $state(false);

  let page = $state<Page>('explorer');
  let search = $state('');
  let showConnection = $state(false);
  let overview = $state<Overview | null>(null);
  let busy = $state(false);
  let error = $state('');
  let activeRequest = $state<string | null>(null);
  let epoch = 0;
  const engineName = $derived(
    connection.engine === 'mysql'
      ? 'MySQL'
      : connection.engine === 'redis'
        ? 'Redis'
        : 'PostgreSQL',
  );
  const isSample = $derived(connection.id === 'sample');
  const selected = $derived(connection.tables.find((t) => t.name === workspace.selectedName));
  const visibleTables = $derived(
    connection.tables.filter((t) => t.name.toLowerCase().includes(search.toLowerCase())),
  );
  const editorSchema = $derived(
    Object.fromEntries(
      connection.tables.map((t) => [`${t.schema}.${t.name}`, t.columns.map((c) => c.name)]),
    ),
  );

  async function loadData(
    table: TableInfo | undefined = selected,
    nextPage = workspace.pageNumber,
    filters = workspace.appliedFilters,
  ) {
    if (!table || busy) return;
    if (dirtyCount || workspace.uncertain) {
      error = 'Save or discard pending edits before refreshing, filtering or changing pages.';
      return;
    }
    workspace.scrollTop = 0;
    error = '';
    workspace.plan = null;
    workspace.pageNumber = nextPage;
    if (isSample) {
      workspace.result = browseSample(
        table,
        filters,
        nextPage,
        workspace.appliedFilterMatch,
        workspace.sortColumn,
        workspace.descending,
      );
      return;
    }
    const current = ++epoch;
    const requestId = crypto.randomUUID();
    busy = true;
    activeRequest = requestId;
    workspace.result = null;
    try {
      const data = await api.browse({
        connectionId: connection.id,
        requestId,
        table: table.name,
        filters,
        filterMatch: workspace.appliedFilterMatch,
        sortColumn: workspace.sortColumn,
        descending: workspace.descending,
        page: nextPage,
      });
      if (current === epoch) workspace.result = data;
    } catch (e) {
      if (current === epoch) error = errorMessage(e);
    } finally {
      if (current === epoch) {
        busy = false;
        activeRequest = null;
      }
    }
  }

  async function selectTable(table: TableInfo) {
    if (busy) return;
    page = 'explorer';
    error = '';
    const id = `${table.schema}.${table.name}`;
    if (openTables.some((t) => t.id === id)) {
      activeId = id;
      return;
    }
    openTables = openTables.filter((t) => t.id !== 'empty');
    openTables.push(createWorkspace(table, connection.engine));
    activeId = id;
    await loadData(table, 0, []);
  }

  function openConnection() {
    if (anyDirty) {
      error = 'Save or discard your pending table edits before changing connections.';
      return;
    }
    showConnection = true;
  }

  function closeTable(id: string, discard = false) {
    if (busy) return;
    const target = openTables.find((t) => t.id === id);
    if (!target) return;
    if (!discard && (Object.keys(target.edits).length || target.uncertain)) {
      closeTarget = id;
      return;
    }
    const index = openTables.findIndex((t) => t.id === id);
    openTables = openTables.filter((t) => t.id !== id);
    if (!openTables.length) openTables = [createWorkspace()];
    if (activeId === id) activeId = openTables[Math.min(index, openTables.length - 1)].id;
    closeTarget = null;
    error = '';
  }

  async function saveChanges() {
    if (busy || !selected || !dirtyCount || workspace.uncertain) return false;
    const rows = changedRows(workspace.result, workspace.edits);
    saving = true;
    busy = true;
    error = '';
    workspace.notice = '';
    try {
      if (isSample) {
        const keys = selected.columns.filter((c) => c.primaryKey).map((c) => c.name);
        for (const edit of rows) {
          const original = sampleRows[selected.name].find((row) =>
            keys.every(
              (key) =>
                row[selected.columns.findIndex((c) => c.name === key)] === edit.original[key],
            ),
          );
          if (original)
            for (const [column, value] of Object.entries(edit.changes))
              original[selected.columns.findIndex((c) => c.name === column)] = value;
        }
      } else {
        await api.save({
          connectionId: connection.id,
          requestId: crypto.randomUUID(),
          table: selected.name,
          rows,
        });
      }
      workspace.edits = {};
      workspace.notice = `${rows.length} row${rows.length === 1 ? '' : 's'} saved${isSample ? ' in the local sample only' : ` to ${engineName}`}.`;
    } catch (e) {
      error = errorMessage(e);
      if (typeof e === 'object' && e && 'code' in e && e.code === 'SAVE_UNCERTAIN')
        workspace.uncertain = true;
      return false;
    } finally {
      saving = false;
      busy = false;
    }
    await loadData();
    return true;
  }

  async function discardChanges() {
    if (busy) return;
    const reconcile = workspace.uncertain;
    workspace.edits = {};
    workspace.uncertain = false;
    workspace.notice = '';
    error = '';
    if (reconcile) await loadData();
  }

  async function saveAndClose() {
    if (!closeTarget) return;
    activeId = closeTarget;
    const id = closeTarget;
    if (await saveChanges()) closeTable(id, true);
    else closeTarget = null;
  }

  onMount(() => {
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (anyDirty || saving || updating) {
        event.preventDefault();
        event.returnValue = '';
      }
    };
    let disposed = false;
    let unlisten = () => {};
    void guardWindowClose(
      () => anyDirty || saving || updating,
      () => {
        page = 'explorer';
        if (!busy)
          activeId =
            openTables.find((t) => Object.keys(t.edits).length || t.uncertain)?.id ?? activeId;
        error = updating
          ? 'Wait for the update to finish before closing.'
          : saving
            ? 'Wait for the save to finish before closing.'
            : 'Save or discard pending edits in your table tabs before closing the app.';
      },
    )
      .then((cleanup) => {
        if (disposed) cleanup();
        else unlisten = cleanup;
      })
      .catch(() => {
        error =
          'The window close guard could not start. Save or discard edits before closing the app.';
      });
    window.addEventListener('beforeunload', beforeUnload);
    return () => {
      disposed = true;
      unlisten();
      window.removeEventListener('beforeunload', beforeUnload);
    };
  });

  async function connected(next: ConnectionInfo) {
    const previousId = connection.id;
    openTables = [createWorkspace()];
    activeId = 'empty';
    connection = next;
    showConnection = false;
    overview = null;
    search = '';
    workspace.result = null;
    workspace.indexes = [];
    error = '';
    if (previousId !== 'sample') await api.disconnect(previousId).catch(() => {});
    if (next.tables[0]) await selectTable(next.tables[0]);
    else {
      workspace.selectedName = '';
      page = 'explorer';
      workspace.tab = 'sql';
      workspace.sql = 'SELECT 1;';
    }
  }

  async function disconnect() {
    if (busy) return;
    if (anyDirty) {
      error = 'Save or discard your pending table edits before disconnecting.';
      return;
    }
    try {
      await api.disconnect(connection.id);
      connection = sampleConnection;
      openTables = [createWorkspace()];
      activeId = 'empty';
      overview = null;
      await selectTable(sampleConnection.tables[0]);
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function selectTab(next: Tab) {
    if (busy) return;
    workspace.tab = next;
    error = '';
    workspace.plan = null;

    if (next === 'data' && !workspace.result) await loadData();
    if (next === 'indexes' && selected) {
      if (isSample) {
        workspace.indexes = sampleIndexes(selected.name);
        return;
      }
      busy = true;
      try {
        workspace.indexes = await api.indexes(connection.id, selected.name);
      } catch (e) {
        error = errorMessage(e);
      } finally {
        busy = false;
      }
    }
  }

  async function runQuery(explain = false) {
    if (busy) return;
    if (isSample) {
      error =
        'Connect your PostgreSQL database to execute SQL. The sample workspace is a local UI preview.';
      return;
    }
    const current = ++epoch;
    const requestId = crypto.randomUUID();
    busy = true;
    activeRequest = requestId;
    error = '';
    workspace.plan = null;
    workspace.queryResult = null;
    try {
      const data = await api.query(connection.id, requestId, workspace.sql, explain);
      if (current === epoch) {
        if (explain) {
          const raw = data.rows[0]?.[0] ?? '{}';
          try {
            workspace.plan = JSON.stringify(JSON.parse(raw), null, 2);
          } catch {
            workspace.plan = raw;
          }
        } else workspace.queryResult = data;
      }
    } catch (e) {
      if (current === epoch) error = errorMessage(e);
    } finally {
      if (current === epoch) {
        busy = false;
        activeRequest = null;
      }
    }
  }

  async function cancel() {
    if (!activeRequest) return;
    try {
      await api.cancel(connection.id, activeRequest);
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function refreshOverview() {
    if (isSample || busy) return;
    busy = true;
    error = '';
    overview = null;
    try {
      overview = await api.overview(connection.id);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function navigate(next: Page) {
    if (busy) return;
    page = next;
    error = '';
    if (next === 'overview' && connection.engine !== 'redis') await refreshOverview();
  }

  async function applyFilters(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    if (dirtyCount || workspace.uncertain) {
      error = 'Save or discard pending edits before applying filters.';
      return;
    }
    workspace.appliedFilters = workspace.filters.map((filter) => ({ ...filter }));
    workspace.appliedFilterMatch = workspace.filterMatch;
    await loadData(selected, 0);
  }

  async function clearFilters() {
    if (busy) return;
    if (dirtyCount || workspace.uncertain) {
      error = 'Save or discard pending edits before clearing filters.';
      return;
    }
    workspace.appliedFilters = [];
    workspace.filters = [
      { column: selected?.columns[0]?.name ?? '', operator: 'equals', value: '' },
    ];
    await loadData(selected, 0);
  }

  async function sortTable(column: string) {
    if (busy) return;
    if (dirtyCount || workspace.uncertain) {
      error = 'Save or discard pending edits before sorting.';
      return;
    }
    workspace.descending = workspace.sortColumn === column ? !workspace.descending : false;
    workspace.sortColumn = column;
    await loadData(selected, 0);
  }

  function handleShortcut(event: KeyboardEvent) {
    if (updating || event.isComposing || document.querySelector('dialog[open]')) return;
    const modifier = event.metaKey || event.ctrlKey;
    if (modifier && event.key.toLowerCase() === 's') {
      event.preventDefault();
      void saveChanges();
    }
    if (
      page === 'explorer' &&
      workspace.tab === 'data' &&
      selected &&
      (event.key === 'F5' || (modifier && event.key.toLowerCase() === 'r'))
    ) {
      event.preventDefault();
      void loadData();
    }
  }
</script>

<svelte:head
  ><title>Table Por · {isSample ? 'Sample workspace' : connection.name}</title></svelte:head
>

<div class="app-shell" inert={updating}>
  <aside class="sidebar">
    <button class="brand" onclick={() => navigate('explorer')} disabled={busy}
      ><span class="brand-mark"><Table2 size={22} /></span><span
        >table<span class="brand-light">por</span><small>DATABASE CLIENT</small></span
      ></button
    >
    <div class="nav-label">WORKSPACE</div>
    <nav aria-label="Workspace navigation">
      <button
        class:active={page === 'explorer'}
        onclick={() => navigate('explorer')}
        disabled={busy}
        ><Database size={17} />{connection.engine === 'redis'
          ? 'Key explorer'
          : 'Database explorer'}{#if connection.engine !== 'redis'}<span class="nav-count"
            >{connection.tables.length}</span
          >{/if}</button
      >
      <button
        class:active={page === 'overview'}
        onclick={() => navigate('overview')}
        disabled={busy}><Activity size={17} />Database overview</button
      >
      <button
        class:active={page === 'assistant'}
        onclick={() => navigate('assistant')}
        disabled={busy}><Sparkles size={17} />AI assistant<span class="soon-tag">NEXT</span></button
      >
    </nav>
    <div class="sidebar-divider"></div>
    <div class="nav-label label-row">
      CONNECTIONS<button
        class="sidebar-icon"
        title="New connection"
        aria-label="New connection"
        onclick={openConnection}
        disabled={busy}><Plus size={16} /></button
      >
    </div>
    <div class="connection-tree">
      <div class="connection-root">
        <ChevronDown size={13} /><span class="db-icon"><Database size={15} /></span><strong
          >{connection.name}</strong
        ><span class={`connection-dot ${isSample ? 'sample-dot' : ''}`}></span>
      </div>
      {#if connection.engine === 'redis'}
        <div class="schema-root">Database {connection.database} · Standalone</div>
        <p class="muted small">
          Browse keys in Key explorer. Server information is available in Database overview.
        </p>
      {:else}
        <div class="schema-root">
          <ChevronDown size={13} /><Columns3 size={14} />{connection.schema}<span
            >{connection.tables.length}</span
          >
        </div>
        <div class="table-search">
          <Search size={13} /><input
            aria-label="Search tables"
            bind:value={search}
            placeholder="Find a table…"
            spellcheck="false"
          />
        </div>
        <div class="table-list">
          {#each visibleTables as table}<button
              class:selected={page === 'explorer' && workspace.selectedName === table.name}
              onclick={() => selectTable(table)}
              disabled={busy}
              ><Table2 size={14} />{table.name}{#if workspace.selectedName === table.name}<span
                  class="selected-indicator"
                ></span>{/if}</button
            >{/each}
          {#if !visibleTables.length}<p class="sidebar-empty">
              {search ? 'No matching tables' : 'No approved base tables'}
            </p>{/if}
        </div>
      {/if}
    </div>
    <div class="sidebar-bottom">
      <ThemeControl />
      <button class:active={page === 'privacy'} onclick={() => navigate('privacy')} disabled={busy}
        ><Settings2 size={16} />Privacy & access<ChevronRight size={14} /></button
      >
      <div class="sidebar-version"><span>Table Por</span><span>v{__APP_VERSION__}</span></div>
    </div>
  </aside>

  <div class="main-shell">
    <header class="topbar">
      <div class="breadcrumbs">
        <PanelLeft size={16} /><span>Workspace</span><ChevronRight size={12} /><strong
          >{page === 'explorer'
            ? 'Database explorer'
            : page === 'overview'
              ? 'Database overview'
              : page === 'privacy'
                ? 'Privacy & access'
                : 'AI assistant'}</strong
        >
      </div>
      <div class="topbar-actions">
        <span class="readonly-indicator"
          ><LockKeyhole size={12} />{connection.manualEdits ? 'Manual editing' : 'Read-only'}</span
        ><button class="secondary-button small-button" onclick={openConnection} disabled={busy}
          ><Plus size={14} />Connect database</button
        >
      </div>
    </header>

    <main class:explorer={page === 'explorer'}>
      {#if isSample}<div class="sample-banner">
          <FlaskConical size={15} /><span
            ><strong>Sample workspace</strong> · Explore the interface with fictional, local data.</span
          ><button onclick={openConnection} disabled={busy}
            >Connect your database<ArrowUpRight size={14} /></button
          >
        </div>{/if}
      {#if error}<div class="error-banner global-error" role="alert">
          <ShieldCheck size={18} /><span>{error}</span><button
            class="icon-button"
            aria-label="Dismiss error"
            onclick={() => (error = '')}><X size={16} /></button
          >
        </div>{/if}

      {#if connection.engine === 'redis' && (page === 'explorer' || page === 'overview')}
        {#key `${connection.id}-${page}`}<RedisWorkspace
            {connection}
            showOverview={page === 'overview'}
            bind:busy
          />{/key}
      {:else if page === 'explorer'}
        <div class="table-tabbar" role="tablist" aria-label="Open tables">
          {#each openTables.filter((t) => t.id !== 'empty') as item (item.id)}
            <div class:active={item.id === activeId} class="table-tab">
              <button
                role="tab"
                aria-selected={item.id === activeId}
                onclick={() => {
                  if (!busy) {
                    activeId = item.id;
                    error = '';
                  }
                }}
                disabled={busy}
              >
                <Table2
                  size={14}
                />{item.selectedName}{#if Object.keys(item.edits).length || item.uncertain}<span
                    class="dirty-dot"
                    aria-label="Unsaved changes"
                  ></span>{/if}
              </button>
              <button
                class="close-tab"
                aria-label={`Close ${item.selectedName} tab`}
                onclick={() => closeTable(item.id)}
                disabled={busy}><X size={13} /></button
              >
            </div>
          {/each}
          {#if openTables[0]?.id === 'empty'}<span class="muted small"
              >Select a table from the sidebar to open it.</span
            >{/if}
        </div>

        <div class="table-context">
          <h1>
            {workspace.selectedName || 'Query workspace'}<span class="object-tag"
              >{selected ? 'TABLE' : 'POSTGRESQL'}</span
            >
          </h1>
          <span class="muted small"
            >{connection.database} / {connection.schema}{#if selected}
              · {selected.columns.length} columns{/if}</span
          >
          <span class="tab-spacer"></span>
          <button
            class="secondary-button"
            aria-label="Refresh table"
            title="Refresh table (⌘/Ctrl+R or F5)"
            onclick={() => loadData()}
            disabled={busy || !selected}><RefreshCw size={13} />Refresh</button
          >
        </div>
        <section class="workbench-panel">
          <div class="tabs" role="tablist" aria-label="Table views">
            <button
              role="tab"
              aria-selected={workspace.tab === 'data'}
              class:current={workspace.tab === 'data'}
              onclick={() => selectTab('data')}
              disabled={busy || !selected}><Table2 size={15} />Data</button
            ><button
              role="tab"
              aria-selected={workspace.tab === 'structure'}
              class:current={workspace.tab === 'structure'}
              onclick={() => selectTab('structure')}
              disabled={busy || !selected}><Columns3 size={15} />Structure</button
            ><button
              role="tab"
              aria-selected={workspace.tab === 'indexes'}
              class:current={workspace.tab === 'indexes'}
              onclick={() => selectTab('indexes')}
              disabled={busy || !selected}><ListTree size={15} />Indexes</button
            ><button
              role="tab"
              aria-selected={workspace.tab === 'sql'}
              class:current={workspace.tab === 'sql'}
              onclick={() => selectTab('sql')}
              disabled={busy}><Code2 size={15} />SQL editor</button
            ><span class="tab-spacer"></span><span class="small muted"
              ><ShieldCheck size={13} />{connection.manualEdits
                ? 'Manual edits enabled'
                : 'Read-only session'}</span
            >
          </div>

          {#if dirtyCount || workspace.uncertain || workspace.notice}
            <div class="save-bar" role="status">
              <span
                >{workspace.notice ||
                  (workspace.uncertain
                    ? 'Save outcome unknown · reload to reconcile'
                    : `${dirtyCount} row${dirtyCount === 1 ? '' : 's'} with unsaved changes`)}</span
              >
              {#if dirtyCount || workspace.uncertain}<div>
                  <button
                    class="secondary-button small-button"
                    onclick={discardChanges}
                    disabled={busy}
                    >{workspace.uncertain ? 'Discard drafts & reload' : 'Discard changes'}</button
                  >
                  <button
                    class="primary-button small-button"
                    onclick={saveChanges}
                    disabled={busy || workspace.uncertain}
                    >{saving ? 'Saving…' : 'Save changes'}</button
                  >
                </div>{/if}
            </div>
          {/if}
          {#if workspace.tab === 'data'}
            <div class="data-toolbar">
              <button
                class="secondary-button"
                class:filter-active={workspace.showFilter}
                onclick={() => (workspace.showFilter = !workspace.showFilter)}
                disabled={busy}
                ><FilterIcon size={13} />Filters{#if workspace.appliedFilters.length}<span
                    class="filter-count">{workspace.appliedFilters.length}</span
                  >{/if}</button
              >
              {#if workspace.appliedFilters.length}<span class="muted small"
                  >{workspace.appliedFilters.length} conditions · {workspace.appliedFilterMatch ===
                  'all'
                    ? 'AND'
                    : 'OR'}</span
                ><button
                  class="icon-button"
                  aria-label="Clear filter"
                  onclick={clearFilters}
                  disabled={busy}><X size={13} /></button
                >{/if}
              <span class="tab-spacer"></span>
              {#if busy && activeRequest}<button class="secondary-button" onclick={cancel}
                  ><Square size={12} />Cancel</button
                >{/if}
              <span class="muted small"
                >{connection.manualEdits &&
                selected?.columns.some((c) => c.editable) &&
                selected?.columns.some((c) => c.primaryKey)
                  ? 'Click to edit · ⌘/Ctrl+S save'
                  : 'Read-only'} · ⌘/Ctrl+R refresh</span
              >
            </div>
            {#if workspace.showFilter && selected}<FilterBuilder
                table={selected}
                bind:filters={workspace.filters}
                bind:match={workspace.filterMatch}
                {busy}
                onapply={applyFilters}
                onclear={clearFilters}
              />{/if}
            {#key workspace.id}<ResultGrid
                result={workspace.result}
                table={selected}
                {busy}
                editable={connection.manualEdits &&
                  !workspace.uncertain &&
                  !!selected?.columns.some((c) => c.primaryKey)}
                edits={workspace.edits}
                bind:top={workspace.scrollTop}
                bind:widths={workspace.columnWidths}
                sortColumn={workspace.sortColumn}
                descending={workspace.descending}
                onsort={sortTable}
                onchange={(row, column, value) => changeCell(workspace, row, column, value)}
              />{/key}
            <div class="result-footer">
              <span
                ><span class="tiny-dot"></span>{busy
                  ? 'Reading…'
                  : `${workspace.result?.rows.length ?? 0} rows shown`}{#if !isSample && workspace.result}<span
                    class="footer-separator">·</span
                  >{workspace.result.elapsedMs} ms{/if}{#if workspace.result?.truncated}<span
                    class="footer-separator">·</span
                  >More rows or byte limit reached{/if}</span
              >
              <div class="pagination">
                <span>Page {workspace.pageNumber + 1}</span><button
                  class="icon-button"
                  aria-label="Previous page"
                  disabled={busy || workspace.pageNumber === 0}
                  onclick={() => loadData(selected, workspace.pageNumber - 1)}
                  ><ChevronLeft size={16} /></button
                ><button
                  class="icon-button"
                  aria-label="Next page"
                  disabled={busy ||
                    !workspace.result?.truncated ||
                    workspace.result.rows.length < 200}
                  onclick={() => loadData(selected, workspace.pageNumber + 1)}
                  ><ChevronRight size={16} /></button
                >
              </div>
            </div>
          {:else if workspace.tab === 'structure' && selected}
            <div class="section-intro">
              <h3>Table structure</h3>
              <p>Column definitions from the approved schema.</p>
            </div>
            <div class="grid-scroll">
              <table class="structure-table">
                <thead
                  ><tr><th>Column</th><th>Data type</th><th>Nullable</th><th>Key</th></tr></thead
                ><tbody
                  >{#each selected.columns as column}<tr
                      ><td class="mono">{column.name}</td><td
                        ><span class="type-chip">{column.dataType}</span></td
                      ><td>{column.nullable ? 'Yes' : 'No'}</td><td
                        >{#if column.primaryKey}<span class="badge">Primary key</span>{:else}<span
                            class="muted">—</span
                          >{/if}</td
                      ></tr
                    >{/each}</tbody
                >
              </table>
            </div>
          {:else if workspace.tab === 'indexes'}
            <div class="section-intro">
              <h3>Indexes</h3>
              <p>
                {isSample
                  ? 'Sample definitions. Usage statistics are not simulated.'
                  : 'Cumulative scan counters. A zero count alone does not mean an index is unnecessary.'}
              </p>
            </div>
            <div class="index-list">
              {#each workspace.indexes as index}<article class="index-card">
                  <div>
                    <ListTree size={18} />
                    <h3>{index.name}</h3>
                    {#if index.primary}<span class="badge">Primary</span
                      >{:else if index.unique}<span class="badge">Unique</span>{/if}<span
                      class="index-metric">{index.size} <span>·</span> {index.scans} scans</span
                    >
                  </div>
                  <pre>{index.definition}</pre>
                </article>{/each}{#if !workspace.indexes.length}<p class="muted">
                  {busy ? 'Reading indexes…' : 'No indexes found.'}
                </p>{/if}
            </div>
          {:else if workspace.tab === 'sql'}
            <div class="editor-toolbar">
              <span class="muted small">One read-only SELECT · schema-qualified tables</span><span
                class="keyboard-hint">⌘ / Ctrl + Enter</span
              >
            </div>
            {#await import('./components/SqlEditor.svelte') then editor}<editor.default
                engine={connection.engine}
                value={workspace.sql}
                onchange={(value) => (workspace.sql = value)}
                onrun={() => runQuery()}
                schema={editorSchema}
              />{/await}
            <div class="query-actions">
              <span class="muted small"
                >Functions, casts, joins and writes are disabled in this preview.</span
              >
              <div>
                {#if busy}<button
                    class="secondary-button small-button"
                    onclick={cancel}
                    disabled={!activeRequest}><Square size={13} />Cancel</button
                  >{:else}<button
                    class="secondary-button small-button"
                    onclick={() => runQuery(true)}>Explain plan</button
                  ><button class="primary-button small-button" onclick={() => runQuery()}
                    ><Play size={13} />Run query</button
                  >{/if}
              </div>
            </div>
            {#if workspace.plan}<div class="plan-result">
                <span class="badge">Estimated plan · query not executed</span>
                <pre>{workspace.plan}</pre>
              </div>{:else}<ResultGrid result={workspace.queryResult} table={selected} {busy} />
              <div class="result-footer">
                <span
                  >{workspace.queryResult
                    ? `${workspace.queryResult.rows.length} rows · ${workspace.queryResult.elapsedMs} ms${workspace.queryResult.truncated ? ' · Result limit reached' : ''}`
                    : 'Results appear here'}</span
                ><span>1,000 rows / 4 MiB maximum</span>
              </div>{/if}
          {/if}
        </section>
        <div class="workspace-footnote">
          <LockKeyhole size={12} />{isSample
            ? 'Sample data is generated in this app. No database is connected.'
            : `Direct connection to ${connection.host}:${connection.port} · ${connection.tls === 'verifyFull' ? 'Verified TLS' : 'Local loopback, unencrypted'} · Session-only credentials`}
        </div>
      {:else if page === 'overview'}
        <div class="page-heading">
          <div>
            <div class="eyebrow">OBSERVABILITY</div>
            <h1>Database overview</h1>
            <p>A snapshot of your database, with the source kept in view.</p>
          </div>
          <button class="secondary-button" onclick={refreshOverview} disabled={busy || isSample}
            ><RefreshCw size={15} />{busy ? 'Reading…' : 'Refresh snapshot'}</button
          >
        </div>
        {#if isSample}<div class="feature-empty">
            <div class="large-icon"><Activity size={30} /></div>
            <h2>Real metrics need a real connection.</h2>
            <p>
              Connect a database to inspect the metrics available for that engine and account. We
              don’t simulate database health.
            </p>
            <button class="primary-button" onclick={openConnection}
              ><PlugZap size={16} />Connect database</button
            >
          </div>{:else if overview}<div class="metric-grid">
            <article>
              <span>Database size</span><strong>{overview.databaseSize}</strong><small
                >Current database</small
              >
            </article>
            <article>
              <span>Connections</span><strong>{overview.connections}</strong><small
                >{overview.activeConnections} active sessions visible to this role</small
              >
            </article>
            <article>
              <span>Buffer cache hit rate</span><strong
                >{overview.cacheHitPercent === null
                  ? 'Unavailable'
                  : `${overview.cacheHitPercent.toFixed(2)}%`}</strong
              ><small>Cumulative · not a health score</small>
            </article>
            <article>
              <span
                >{connection.engine === 'mysql'
                  ? 'COMMIT statements'
                  : 'Transactions committed'}</span
              ><strong>{overview.transactionsCommitted}</strong><small
                >{overview.transactionsRolledBack}
                {connection.engine === 'mysql'
                  ? 'ROLLBACK statements · server-wide; excludes autocommit'
                  : 'rolled back'}</small
              >
            </article>
          </div>
          <div class="detail-card">
            <h3>About this snapshot</h3>
            <dl>
              <dt>Source</dt>
              <dd>pg_stat_database · pg_stat_activity</dd>
              <dt>Observed at</dt>
              <dd>{overview.observedAt}</dd>
              <dt>Statistics reset</dt>
              <dd>{overview.statsReset ?? 'Not reported'}</dd>
              <dt>Server</dt>
              <dd>{engineName} {overview.serverVersion}</dd>
              <dt>Collection</dt>
              <dd>Manual refresh. Current activity can miss short queries.</dd>
            </dl>
            <p class="muted small">
              Activity visibility depends on database permissions. This is not a complete query
              history. The collector itself can appear as an active session.
            </p>
          </div>{:else if busy}<div class="feature-empty">
            <Activity class="spinning" size={24} />
            <p>Collecting a fresh snapshot…</p>
          </div>{/if}
      {:else if page === 'privacy'}
        <div class="page-heading">
          <div>
            <div class="eyebrow">BUILT TO STAY LOCAL</div>
            <h1>Privacy & access</h1>
            <p>Clear boundaries for your data and your database.</p>
          </div>
          <ShieldCheck size={36} class="privacy-hero-icon" />
        </div>
        <div class="privacy-grid">
          <article class="detail-card">
            <LockKeyhole size={22} />
            <h3>Explicit saves, protected queries</h3>
            <p>
              SQL is checked in Rust and runs in read-only transactions. Redis commands use a read
              allowlist. PostgreSQL manual cell saves use a separate, permission-checked UPDATE
              operation with conflict detection.
            </p>
            <span class="badge">No delete or truncate tools</span>
          </article>
          <article class="detail-card">
            <Database size={22} />
            <h3>Direct database access</h3>
            <p>
              Database connections go directly to the endpoint you enter. Signed releases can check
              GitHub for updates; database contents and credentials are never included. Turn off
              automatic checks in the update menu to stop background update requests. There are no
              analytics, cloud AI or remote assets.
            </p>
            <span class="badge">No database uploads</span>
          </article>
          <article class="detail-card">
            <ShieldCheck size={22} />
            <h3>Session-only storage</h3>
            <p>
              Credentials, query text and results are held in memory. This preview does not save
              connections, history or result caches to disk. Only appearance and update preferences
              are stored locally.
            </p>
            <span class="badge">Database persistence off</span>
          </article>
          <article class="detail-card">
            <CircleHelp size={22} />
            <h3>Preview boundaries</h3>
            <p>
              PostgreSQL and MySQL expose supported base tables; Redis exposes approved read
              commands. Views, custom routines, RLS tables and advanced SQL are intentionally
              excluded.
            </p>
            <span class="neutral-badge">Security hardening in progress</span>
          </article>
        </div>
        <div class="detail-card connection-details">
          <h3>Current session</h3>
          <dl>
            <dt>Connection</dt>
            <dd>{isSample ? 'None · sample workspace' : connection.name}</dd>
            <dt>Allowed endpoint</dt>
            <dd>
              {isSample ? 'No database network access' : `${connection.host}:${connection.port}`}
            </dd>
            <dt>Transport</dt>
            <dd>
              {isSample
                ? 'Not applicable'
                : connection.tls === 'verifyFull'
                  ? 'Verified TLS, minimum TLS 1.2'
                  : 'Unencrypted · literal loopback IP only'}
            </dd>
            <dt>Reader</dt>
            <dd>{isSample ? 'Not applicable' : connection.username}</dd>
          </dl>
          {#if !isSample}<button class="secondary-button" onclick={disconnect} disabled={busy}
              ><Unplug size={15} />Disconnect and forget credentials</button
            >{/if}
        </div>
      {:else}
        <div class="page-heading">
          <div>
            <div class="eyebrow">LOCAL INTELLIGENCE</div>
            <h1>Your database, explained.</h1>
            <p>An assistant that follows the same access rules as your workspace.</p>
          </div>
          <span class="neutral-badge">Planned · not enabled</span>
        </div>
        <div class="feature-empty ai-empty">
          <div class="large-icon"><Sparkles size={30} /></div>
          <h2>Local AI is the next chapter.</h2>
          <p>
            First, we’re building a dependable read-only foundation. The assistant will use a model
            on your device, scoped context and the same query policy.
          </p>
          <div class="ai-principles">
            <span><Check size={15} />No cloud fallback</span><span
              ><Check size={15} />No database credentials</span
            ><span><Check size={15} />No write tools</span>
          </div>
        </div>
      {/if}
    </main>
    <footer class="app-status">
      <span
        ><span class="tiny-dot"></span>{isSample
          ? 'Sample mode · offline'
          : `${engineName} · direct session`}</span
      ><span
        >{desktopAvailable ? 'Desktop' : 'Browser preview'}<span class="footer-separator">·</span
        >All results stay in this app</span
      >
    </footer>
  </div>
</div>
{#if showConnection}<ConnectionDialog
    onconnected={connected}
    onclose={() => (showConnection = false)}
  />{/if}

<UpdateNotice blocked={busy || anyDirty || showConnection} bind:installing={updating} />

<svelte:window onkeydown={handleShortcut} />
{#if closeTarget}<ConfirmClose
    name={openTables.find((t) => t.id === closeTarget)?.selectedName ?? 'table'}
    onsave={saveAndClose}
    ondiscard={() => closeTable(closeTarget!, true)}
    oncancel={() => (closeTarget = null)}
    {busy}
  />{/if}
