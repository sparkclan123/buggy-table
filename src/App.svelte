<script lang="ts">
  import brandIcon from '../assets/buggy-table-icon.png';
  import {
    Database,
    Table2,
    Plus,
    Search,
    ChevronRight,
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
    ChevronUp,
    ChevronDown,
    Trash2,
    WrapText,
  } from '@lucide/svelte';
  import ThemeControl from './components/ThemeControl.svelte';
  import UpdateNotice from './components/UpdateNotice.svelte';
  import TableTabs from './components/TableTabs.svelte';
  let updating = $state(false);
  import { onMount } from 'svelte';
  import { guardWindowClose, listenForTabClose } from './lib/window-lifecycle';
  import {
    tableKey,
    createWorkspace,
    changeCell,
    changedRows,
    pendingRows,
    beginCellEdit,
    endCellEdit,
    clearHistory,
    setDeletedRows,
    appendRows,
    undoDraft,
    type DraftRow,
    type TableView as Tab,
  } from './lib/workspace';
  import RedisWorkspace from './components/RedisWorkspace.svelte';
  import ConfirmClose from './components/ConfirmClose.svelte';
  import ConnectionDialog from './components/ConnectionDialog.svelte';
  import ConnectionManager from './components/ConnectionManager.svelte';
  import VpnControls from './components/VpnControls.svelte';
  import VpnDialog from './components/VpnDialog.svelte';
  import type { VpnSelection, VpnStatus } from './lib/types';
  import { createRedisWorkspace } from './lib/redis-workspace';
  import {
    connectionWorkspace,
    savedWorkspace,
    hasEdits,
    layoutOf,
    type ConnectionWorkspace,
  } from './lib/connections';
  import ResultGrid from './components/ResultGrid.svelte';
  import ResultTiming from './components/ResultTiming.svelte';
  import { readInspectorPreferences, saveInspectorPreferences } from './lib/row-inspector';
  import SnapshotChart from './components/SnapshotChart.svelte';
  import { numeric, type SnapshotPoint } from './lib/stats';
  import FilterBuilder from './components/FilterBuilder.svelte';
  import { api, errorMessage, desktopAvailable } from './lib/api';
  import { browseSample, sampleConnection, sampleIndexes, sampleRows } from './lib/sample';
  import type { ConnectedProfile, SavedConnection, TableInfo } from './lib/types';

  const initialEntry = connectionWorkspace(sampleConnection);
  initialEntry.tabs = [createWorkspace(sampleConnection.tables[0])];
  initialEntry.tabs[0].result = browseSample(sampleConnection.tables[0], [], 0);
  initialEntry.activeId = initialEntry.tabs[0].id;
  let entries = $state<ConnectionWorkspace[]>([initialEntry]);
  let activeKey = $state('sample');
  const session = $derived(entries.find((e) => e.key === activeKey)!);
  const connection = $derived(session.info);
  const openTables = $derived(session.tabs);
  const activeId = $derived(session.activeId);
  const workspace = $derived(openTables.find((t) => t.id === activeId)!);
  let closeTarget = $state<string | null>(null);
  const dirtyCount = $derived(pendingRows(workspace));
  let counting = $state(false);
  const anyDirty = $derived(entries.some(hasEdits));
  const page = $derived(session.page);
  const search = $derived(session.search);
  const overview = $derived(session.overview);
  let saving = $state(false);
  let showConnection = $state(false);
  let connectionInitial = $state<SavedConnection | null>(null);
  let showManager = $state(false);
  let vpnOwner = $state<string | null>(null);
  const vpnEntry = $derived(entries.find((e) => (e.saved?.id ?? e.info.vpnOwner) === vpnOwner));
  const quickConnections = $derived(
    entries
      .filter((e) => e.key !== 'sample')
      .sort(
        (a, b) =>
          Number(b.saved?.pinned ?? false) - Number(a.saved?.pinned ?? false) ||
          (b.saved?.lastUsed ?? 0) - (a.saved?.lastUsed ?? 0),
      )
      .slice(0, 5),
  );
  let busy = $state(false);
  let error = $state('');
  let inspector = $state(readInspectorPreferences());
  function updateInspector(next: Partial<typeof inspector>) {
    inspector = { ...inspector, ...next };
    if (!saveInspectorPreferences(inspector))
      error = 'Row details changed for this session, but local preferences could not be saved.';
  }
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
  const untestedPostgres = $derived(
    !isSample && connection.engine === 'postgres' && Number(connection.serverVersion) >= 190000,
  );
  const selected = $derived(
    connection.tables.find(
      (t) => t.name === workspace.selectedName && t.schema === workspace.schema,
    ),
  );
  const visibleTables = $derived(
    connection.tables.filter(
      (t) => t.schema === session.navSchema && t.name.toLowerCase().includes(search.toLowerCase()),
    ),
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
    if (!table || busy || !session.online) return;
    if (table.readBlockedReason) {
      workspace.tab = 'structure';
      error = table.readBlockedReason;
      return;
    }
    if (dirtyCount || workspace.uncertain) {
      error = 'Save or discard pending edits before refreshing, filtering or changing pages.';
      return;
    }
    workspace.scrollTop = 0;
    clearHistory(workspace);
    error = '';
    workspace.plan = null;
    const countKey = JSON.stringify([
      table.schema,
      table.name,
      filters,
      workspace.appliedFilterMatch,
    ]);
    if (workspace.countKey !== countKey) {
      workspace.exactCount = null;
      workspace.countKey = countKey;
      workspace.countNotice = '';
    }
    workspace.pageNumber = nextPage;
    workspace.selectedRows = [];
    if (isSample) {
      workspace.result = browseSample(
        table,
        filters,
        nextPage,
        workspace.appliedFilterMatch,
        workspace.sortColumn,
        workspace.descending,
        workspace.pageSize,
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
        schema: table.schema,
        requestId,
        table: table.name,
        filters,
        filterMatch: workspace.appliedFilterMatch,
        sortColumn: workspace.sortColumn,
        descending: workspace.descending,
        page: nextPage,
        pageSize: workspace.pageSize,
      });
      if (current === epoch) {
        workspace.result = data;
        trimResults();
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

  async function selectTable(table: TableInfo) {
    if (busy) return;
    session.page = 'explorer';
    error = '';
    const id = tableKey(table.schema, table.name);
    if (openTables.some((t) => t.id === id)) {
      await activateTab(id);
      return;
    }
    session.tabs = openTables.filter((t) => t.id !== 'empty');
    if (openTables.length >= 40) {
      error = 'Close an unused table tab first (maximum 40 per connection).';
      return;
    }
    openTables.push(createWorkspace(table, connection.engine));
    session.activeId = id;
    void persistLayout(session);
    if (!table.readBlockedReason) await loadData(table, 0, []);
  }

  function openConnection() {
    if (busy) return;
    connectionInitial = null;
    showManager = false;
    showConnection = true;
  }

  function closeTable(id: string, discard = false) {
    if (busy) return;
    const target = openTables.find((t) => t.id === id);
    if (!target) return;
    if (!discard && (pendingRows(target) || target.uncertain)) {
      closeTarget = id;
      return;
    }
    const wasActive = activeId === id;
    const index = openTables.findIndex((t) => t.id === id);
    session.tabs = openTables.filter((t) => t.id !== id);
    if (!openTables.length) session.tabs = [createWorkspace()];
    if (activeId === id) session.activeId = openTables[Math.min(index, openTables.length - 1)].id;
    closeTarget = null;
    if (wasActive && session.online) void activateTab(session.activeId);
    void persistLayout(session);
    error = '';
  }

  async function saveChanges() {
    if (busy || !session.online || !selected || !dirtyCount || workspace.uncertain) return false;
    endCellEdit(workspace);
    const rows = changedRows(
      workspace.result,
      workspace.edits,
      workspace.deletedRows,
      workspace.insertedRows,
    );
    saving = true;
    busy = true;
    error = '';
    workspace.notice = '';
    try {
      if (isSample) {
        const keys = selected.columns.filter((c) => c.primaryKey).map((c) => c.name);
        for (const edit of rows) {
          if (edit.insert) {
            const nextId = String(
              Math.max(0, ...sampleRows[selected.name].map((r) => Number(r[0]))) + 1,
            );
            sampleRows[selected.name].push(
              selected.columns.map((c) => (c.primaryKey ? nextId : (edit.changes[c.name] ?? null))),
            );
            continue;
          }
          const original = sampleRows[selected.name].find((row) =>
            keys.every(
              (key) =>
                row[selected.columns.findIndex((c) => c.name === key)] === edit.original[key],
            ),
          );
          if (original && edit.delete)
            sampleRows[selected.name] = sampleRows[selected.name].filter((row) => row !== original);
          else if (original)
            for (const [column, value] of Object.entries(edit.changes))
              original[selected.columns.findIndex((c) => c.name === column)] = value;
        }
      } else if (rows.length) {
        await api.save({
          connectionId: connection.id,
          schema: selected.schema,
          requestId: crypto.randomUUID(),
          table: selected.name,
          rows,
        });
      }
      workspace.edits = {};
      workspace.insertedRows = [];
      clearHistory(workspace);
      workspace.deletedRows = [];
      workspace.selectedRows = [];
      workspace.exactCount = null;
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
    workspace.insertedRows = [];
    clearHistory(workspace);
    workspace.deletedRows = [];
    workspace.selectedRows = [];
    workspace.uncertain = false;
    workspace.notice = '';
    error = '';
    if (reconcile) await loadData();
  }

  async function saveAndClose() {
    if (!closeTarget) return;
    session.activeId = closeTarget;
    const id = closeTarget;
    if (await saveChanges()) closeTable(id, true);
    else closeTarget = null;
  }

  onMount(() => {
    let checkingVpn = false;
    const vpnTimer = window.setInterval(async () => {
      if (checkingVpn || !desktopAvailable || document.hidden) return;
      checkingVpn = true;
      try {
        await Promise.all(
          entries
            .filter((e) => e.online && e.info.vpn?.enabled)
            .map(async (e) => {
              try {
                e.vpnStatus = await api.vpnStatus(e.info.id);
              } catch {
                /* Keep the last known status; never silently select direct routing. */
              }
            }),
        );
      } finally {
        checkingVpn = false;
      }
    }, 15_000);
    if (desktopAvailable)
      void api
        .connections()
        .then((profiles) => {
          if (!profiles?.length) return;
          for (const profile of profiles)
            if (!entries.some((e) => e.key === profile.id)) entries.push(savedWorkspace(profile));
          if (activeKey === 'sample' && !hasEdits(session) && !showConnection)
            activeKey = profiles.sort((a, b) => b.lastUsed - a.lastUsed)[0].id;
        })
        .catch((e) => {
          error = errorMessage(e);
        });
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (anyDirty || saving || updating) {
        event.preventDefault();
        event.returnValue = '';
      }
    };
    let disposed = false;
    let unlisten = () => {};
    let unlistenTab = () => {};
    void listenForTabClose(requestTabClose)
      .then((cleanup) => {
        if (disposed) cleanup();
        else unlistenTab = cleanup;
      })
      .catch(() => {
        error = 'The native close-tab shortcut could not start. Use the tab close button.';
      });
    void guardWindowClose(
      () => anyDirty || saving || updating,
      () => {
        if (!busy && !updating) {
          const pending = entries.find(hasEdits);
          if (pending) activeKey = pending.key;
        }
        session.page = 'explorer';
        if (!busy)
          session.activeId = openTables.find((t) => pendingRows(t) || t.uncertain)?.id ?? activeId;
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
      window.clearInterval(vpnTimer);
      disposed = true;
      unlisten();
      unlistenTab();
      window.removeEventListener('beforeunload', beforeUnload);
    };
  });

  function trimResults() {
    workspace.lastAccess = Date.now();
    const cached = entries
      .flatMap((e) => e.tabs)
      .filter(
        (t) => t !== workspace && !pendingRows(t) && !t.uncertain && (t.result || t.queryResult),
      )
      .sort((a, b) => b.lastAccess - a.lastAccess);
    for (const tab of cached.slice(5)) {
      clearHistory(tab);
      tab.result = null;
      tab.queryResult = null;
      tab.plan = null;
      tab.indexes = [];
    }
  }

  const layoutWrites = new Map<string, Promise<void>>();
  async function persistLayout(entry: ConnectionWorkspace) {
    if (!entry.saved) return;
    const layout = layoutOf(entry);
    entry.saved.layout = layout;
    const id = entry.saved.id,
      pinned = entry.saved.pinned,
      group = entry.saved.group;
    const pending = (layoutWrites.get(id) ?? Promise.resolve())
      .then(() => api.updateConnection(id, layout, pinned, group))
      .catch((e) => {
        error = `Connection settings were not saved. ${errorMessage(e)}`;
      });
    layoutWrites.set(id, pending);
    await pending;
    if (layoutWrites.get(id) === pending) layoutWrites.delete(id);
  }

  function switchConnection(entry: ConnectionWorkspace) {
    if (busy) return;
    activeKey = entry.key;
    showManager = false;
    error = '';
    closeTarget = null;
    if (entry.saved) {
      entry.saved.lastUsed = Math.floor(Date.now() / 1000);
      void persistLayout(entry);
    }
    if (entry.online && workspace.tab === 'data' && !workspace.result) void loadData();
  }

  async function ensureSchema(schema: string, refresh = false) {
    if (
      isSample ||
      connection.engine !== 'postgres' ||
      (!refresh && session.loadedSchemas.includes(schema))
    )
      return;
    const tables = await api.schemaTables(connection.id, schema);
    session.info.tables = [...connection.tables.filter((t) => t.schema !== schema), ...tables];
    if (!session.loadedSchemas.includes(schema)) session.loadedSchemas.push(schema);
  }

  async function activateTab(id: string) {
    if (busy || !session.online) return;
    session.activeId = id;
    workspace.lastAccess = Date.now();
    error = '';
    try {
      busy = true;
      await ensureSchema(workspace.schema || session.navSchema);
      if (workspace.schema) session.navSchema = workspace.schema;
      if (!workspace.filters[0]?.column && selected)
        workspace.filters = [
          { column: selected.columns[0]?.name ?? '', operator: 'equals', value: '' },
        ];
    } catch (e) {
      error = errorMessage(e);
      return;
    } finally {
      busy = false;
    }
    void persistLayout(session);
    if (workspace.tab === 'data' && !workspace.result) await loadData();
  }

  async function changeSchema(schema: string) {
    if (busy) return;
    busy = true;
    error = '';
    try {
      await ensureSchema(schema, true);
      session.navSchema = schema;
      if (!workspace.selectedName) workspace.schema = schema;
      session.search = '';
      void persistLayout(session);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function connected(result: ConnectedProfile) {
    const next = result.connection;
    const existing = entries.find(
      (e) =>
        e.key === (result.profile?.id ?? connectionInitial?.id) ||
        (!!next.vpnOwner && e.info.vpnOwner === next.vpnOwner),
    );
    const entry = connectionWorkspace(next, result.profile ?? existing?.saved ?? null);
    if (existing) entries = entries.map((e) => (e.key === existing.key ? entry : e));
    else entries.push(entry);
    activeKey = entry.key;
    if (next.vpn?.enabled)
      void api
        .vpnStatus(next.id)
        .then((status) => {
          const current = entries.find((e) => e.info.id === entry.info.id);
          if (current) current.vpnStatus = status;
        })
        .catch(() => {});
    showConnection = false;
    showManager = false;
    error = result.warning ?? '';
    let schemaWarning = '';
    if (next.engine === 'postgres') {
      busy = true;
      try {
        session.schemas = (await api.schemas(next.id)) ?? [next.schema];
      } catch (e) {
        schemaWarning = errorMessage(e);
      } finally {
        busy = false;
      }
    }
    if (entry.tabs[0].id === 'empty') {
      entry.tabs[0].schema = entry.navSchema;
      if (next.tables[0]) await selectTable(next.tables[0]);
    } else await activateTab(entry.activeId);
    if (entry.saved) void persistLayout(entry);
    if (result.warning || schemaWarning)
      error = [result.warning, schemaWarning].filter(Boolean).join(' ');
  }

  async function reconnect(entry: ConnectionWorkspace) {
    if (busy) return;
    switchConnection(entry);
    connectionInitial = entry.saved;
    if (!entry.saved?.hasPassword) {
      showConnection = true;
      return;
    }
    busy = true;
    try {
      const result = await api.reconnect(entry.saved.id);
      busy = false;
      await connected(result);
    } catch (e) {
      error = errorMessage(e);
      showConnection = true;
    } finally {
      busy = false;
    }
  }

  async function disconnect() {
    if (busy || !session.online || isSample) return;
    if (hasEdits(session)) {
      error = 'Save or discard this connection’s pending edits before disconnecting.';
      return;
    }
    busy = true;
    try {
      await api.disconnect(connection.id);
      await persistLayout(session);
      session.online = false;
      session.overview = null;
      session.redis = createRedisWorkspace();
      // Keep table identities, but drop cached results and SQL text after disconnect.
      session.tabs = session.tabs.map((t) =>
        createWorkspace(
          t.selectedName
            ? { schema: t.schema, name: t.selectedName, columns: [], estimatedRows: null }
            : undefined,
          connection.engine,
        ),
      );
      session.info.tables = [];
      session.loadedSchemas = [];
      if (!session.saved) {
        const key = session.key;
        if (session.info.vpnOwner) {
          try {
            await api.discardVpnDraft(session.info.vpnOwner);
          } catch (e) {
            error = errorMessage(e);
          }
        }
        entries = entries.filter((e) => e.key !== key);
        activeKey = entries.find((e) => e.key !== 'sample')?.key ?? 'sample';
      }
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  function vpnSelection(entry: ConnectionWorkspace): VpnSelection {
    if (entry.online)
      return {
        enabled: entry.info.vpn?.enabled ?? false,
        profileId: entry.info.vpn?.profileId ?? entry.saved?.vpn?.profileId ?? null,
      };
    return entry.saved?.vpn ?? entry.info.vpn ?? { enabled: false, profileId: null };
  }
  function openVpn(entry: ConnectionWorkspace) {
    if (busy) return;
    const owner = entry.saved?.id ?? entry.info.vpnOwner;
    if (!owner) {
      error = 'Reconnect this database to configure VPN settings.';
      return;
    }
    showManager = false;
    vpnOwner = owner;
    if (entry.online)
      void api
        .vpnStatus(entry.info.id)
        .then((status) => (entry.vpnStatus = status))
        .catch((e) => (error = errorMessage(e)));
  }
  async function changeVpn(entry: ConnectionWorkspace, selection: VpnSelection) {
    if (busy) throw { message: 'Wait for the current operation to finish.' };
    if (hasEdits(entry))
      throw {
        message:
          'Save or discard this connection’s pending edits before changing or reconnecting VPN.',
      };
    const owner = entry.saved?.id ?? entry.info.vpnOwner;
    if (!owner) throw { message: 'Reconnect this database before configuring VPN.' };
    busy = true;
    error = '';
    try {
      await persistLayout(entry);
      const changed = await api.setConnectionVpn(
        owner,
        selection,
        entry.online ? entry.info.id : null,
      );
      if (changed.connection) {
        busy = false;
        await connected({
          connection: changed.connection,
          profile: changed.profile ?? entry.saved,
          warning: changed.warning,
        });
      } else if (changed.profile) {
        entry.saved = changed.profile;
        entry.info.vpn = selection;
      }
    } finally {
      busy = false;
    }
  }
  function toggleVpn(entry: ConnectionWorkspace) {
    const current = vpnSelection(entry);
    if (!current.profileId) {
      openVpn(entry);
      return;
    }
    void changeVpn(entry, { ...current, enabled: !current.enabled }).catch(
      (e) => (error = errorMessage(e)),
    );
  }
  async function testVpn(entry: ConnectionWorkspace): Promise<VpnStatus | null> {
    if (!entry.online) return null;
    entry.vpnStatus = await api.vpnStatus(entry.info.id, true);
    return entry.vpnStatus;
  }
  function refreshVpn(entry: ConnectionWorkspace) {
    return changeVpn(entry, vpnSelection(entry));
  }

  async function pinConnection(entry: ConnectionWorkspace) {
    if (!entry.saved || busy) return;
    entry.saved.pinned = !entry.saved.pinned;
    await persistLayout(entry);
  }
  function editConnection(entry: ConnectionWorkspace) {
    if (entry.online || busy) return;
    switchConnection(entry);
    connectionInitial = entry.saved;
    showConnection = true;
  }
  async function removeConnection(entry: ConnectionWorkspace) {
    if (!entry.saved || entry.online || hasEdits(entry) || busy) return;
    busy = true;
    try {
      await api.removeConnection(entry.saved.id);
      entries = entries.filter((e) => e.key !== entry.key);
      if (activeKey === entry.key)
        activeKey = entries.find((e) => e.key !== 'sample')?.key ?? 'sample';
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
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
        workspace.indexes = await api.indexes(connection.id, selected.name, selected.schema);
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
      const data = await api.query(
        connection.id,
        requestId,
        workspace.sql,
        explain,
        workspace.schema || session.navSchema,
      );
      if (current === epoch) {
        if (explain) {
          const raw = data.rows[0]?.[0] ?? '{}';
          try {
            workspace.plan = JSON.stringify(JSON.parse(raw), null, 2);
          } catch {
            workspace.plan = raw;
          }
        } else workspace.queryResult = data;
        trimResults();
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
    if (isSample || busy || !session.online) return;
    busy = true;
    error = '';
    try {
      session.overview = await api.overview(connection.id);
      const previous = session.snapshots.at(-1);
      if (
        previous &&
        (previous.statsReset !== session.overview.statsReset ||
          numeric(session.overview.transactionsCommitted)! <
            numeric(previous.transactionsCommitted)!)
      )
        session.snapshots = [];
      session.snapshots = [...session.snapshots.slice(-29), session.overview];
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  function metricPoints(key: keyof NonNullable<typeof overview>): SnapshotPoint[] {
    return session.snapshots.flatMap((snapshot) => {
      const value = numeric(snapshot[key]);
      return value === null ? [] : [{ value, at: snapshot.observedAt }];
    });
  }

  async function navigate(next: ConnectionWorkspace['page']) {
    if (busy) return;
    session.page = next;
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

  function stageDelete(rows: number[]) {
    if (busy || !selected || workspace.uncertain) return;
    if (
      !connection.manualEdits ||
      (!selected.deletable && rows.some((i) => i < (workspace.result?.rows.length ?? 0)))
    ) {
      error =
        'This table needs a primary key and DELETE permission in a manual-edit session to delete rows.';
      return;
    }
    const next = [...new Set([...workspace.deletedRows, ...rows])];
    if (
      new Set([
        ...next.filter((i) => i < (workspace.result?.rows.length ?? 0)),
        ...Object.keys(workspace.edits).map(Number),
      ]).size +
        workspace.insertedRows.length >
      100
    ) {
      error = 'Save at most 100 changed or deleted rows at once.';
      return;
    }
    setDeletedRows(workspace, next);
    workspace.notice = '';
    error = '';
  }

  function pasteRows(rows: DraftRow[]) {
    if (busy || !session.online || !selected || !workspace.result || workspace.uncertain) return;
    if (!connection.manualEdits || !selected.insertable) {
      error =
        'Copying into a new row requires INSERT permission in a manual-edit PostgreSQL session.';
      return;
    }
    const drafts = rows.map((row) =>
      Object.fromEntries(
        selected.columns
          .filter((c) => c.insertable && !c.primaryKey && Object.hasOwn(row, c.name))
          .map((c) => [c.name, row[c.name]]),
      ),
    );
    const pending = changedRows(workspace.result, workspace.edits, workspace.deletedRows, [
      ...workspace.insertedRows,
      ...drafts,
    ]);
    if (
      pendingRows(workspace) + drafts.length > 100 ||
      pending.reduce((n, r) => n + Object.keys(r.changes).length, 0) > 500 ||
      new TextEncoder().encode(JSON.stringify(pending)).length > 4 * 1024 * 1024
    ) {
      error = 'Save at most 100 changed rows, 500 changed cells and 4 MiB at once.';
      return;
    }
    appendRows(workspace, drafts);
    workspace.notice = `${drafts.length} new row(s) appended below this page. Primary keys use database defaults; fill any Required cells before Save.`;
    error = '';
  }

  async function changePageSize(value: number) {
    if (busy) return;
    if (dirtyCount || workspace.uncertain) {
      error = 'Save or discard pending edits before changing page size.';
      return;
    }
    workspace.pageSize = value;
    await loadData(selected, 0);
  }

  async function countRows() {
    if (busy || !selected || !session.online) return;
    workspace.countNotice = '';
    if (isSample) {
      workspace.exactCount = String(
        browseSample(
          selected,
          workspace.appliedFilters,
          0,
          workspace.appliedFilterMatch,
          null,
          false,
          sampleRows[selected.name].length || 1,
        ).rows.length,
      );
      return;
    }
    counting = true;
    busy = true;
    const requestId = crypto.randomUUID();
    activeRequest = requestId;
    try {
      const count = await api.count({
        connectionId: connection.id,
        schema: selected.schema,
        requestId,
        table: selected.name,
        filters: workspace.appliedFilters,
        filterMatch: workspace.appliedFilterMatch,
        sortColumn: null,
        descending: false,
        page: 0,
        pageSize: workspace.pageSize,
      });
      workspace.exactCount = count.rows;
      workspace.countNotice = `Counted in ${count.elapsedMs} ms · snapshot at count time`;
    } catch (e) {
      workspace.countNotice = errorMessage(e);
    } finally {
      busy = false;
      counting = false;
      activeRequest = null;
    }
  }

  let lastTabClose = 0;
  function requestTabClose() {
    if (updating || busy || document.querySelector('dialog[open]')) return;
    if (Date.now() - lastTabClose < 100) return;
    lastTabClose = Date.now();
    if (page === 'explorer' && connection.engine !== 'redis' && workspace.id !== 'empty')
      closeTable(activeId);
  }

  function handleShortcut(event: KeyboardEvent) {
    if (updating || event.isComposing || document.querySelector('dialog[open]')) return;
    const modifier = event.metaKey || event.ctrlKey;
    if (modifier && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      if (!busy) showManager = true;
      return;
    }
    if (modifier && event.key.toLowerCase() === 'w') {
      event.preventDefault();
      requestTabClose();
      return;
    }
    if (!session.online) return;
    if (
      modifier &&
      event.key.toLowerCase() === 'z' &&
      page === 'explorer' &&
      workspace.tab === 'data' &&
      (event.target as Element)?.closest('.result-area')
    ) {
      event.preventDefault();
      if (!busy) undoDraft(workspace, event.shiftKey);
      return;
    }
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
  ><title>buggy-table · {isSample ? 'Sample workspace' : connection.name}</title></svelte:head
>

<div class="app-shell" inert={updating}>
  <aside class="sidebar">
    <button class="brand" onclick={() => navigate('explorer')} disabled={busy}
      ><img class="brand-mark" src={brandIcon} alt="" width="32" height="32" /><span
        >buggy-table<small>DATABASE CLIENT</small></span
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
    <button
      class="connection-switcher"
      onclick={() => (showManager = true)}
      disabled={busy}
      aria-label="Search or switch connections"
      ><Search size={13} />Find connection…<kbd>⌘/Ctrl K</kbd></button
    >
    {#if quickConnections.length}<div
        class="quick-connections"
        aria-label="Pinned and recent connections"
      >
        {#each quickConnections as entry (entry.key)}<div
            class:current={entry.key === activeKey}
            class="quick-connection"
          >
            <button
              onclick={() => switchConnection(entry)}
              disabled={busy}
              title={`${entry.info.name} · ${entry.info.host} / ${entry.info.database}`}
              aria-label={`Switch to ${entry.info.name}`}
              ><span class="engine-code"
                >{entry.info.engine === 'postgres'
                  ? 'PG'
                  : entry.info.engine === 'mysql'
                    ? 'MY'
                    : 'RD'}</span
              ><span
                >{entry.saved?.pinned ? '★ ' : ''}{entry.info.name}{hasEdits(entry)
                  ? ' •'
                  : ''}</span
              ></button
            >
            {#if !entry.online}<button
                class="quick-reconnect"
                title="Reconnect"
                aria-label={`Reconnect ${entry.info.name}`}
                onclick={() => reconnect(entry)}
                disabled={busy}><PlugZap size={13} /></button
              >{:else}<span class="tiny-dot" title="Session ready"></span>{/if}
          </div>{/each}
        <button class="all-connections" onclick={() => (showManager = true)} disabled={busy}
          >All connections ({entries.length - 1})</button
        >
      </div>{/if}
    <div class="connection-tree">
      <div class="connection-root">
        <span class="db-icon"><Database size={14} /></span><strong title={connection.name}
          >{connection.name}</strong
        >
        {#if !isSample && session.online}<button
            class="icon-button"
            aria-label="Disconnect current connection"
            title="Disconnect"
            onclick={disconnect}
            disabled={busy}><Unplug size={13} /></button
          >{/if}
      </div>
      {#if !isSample}<div class="vpn-sidebar-line">
          <VpnControls
            enabled={vpnSelection(session).enabled}
            label={connection.name}
            status={session.vpnStatus}
            disabled={busy}
            onsettings={() => openVpn(session)}
            ontoggle={() => toggleVpn(session)}
            onrefresh={session.online
              ? () => {
                  void refreshVpn(session).catch((e) => (error = errorMessage(e)));
                }
              : undefined}
          /><span class="vpn-sidebar-state" title={session.vpnStatus?.message ?? ''}
            >{!session.online
              ? 'Offline'
              : !connection.vpn?.enabled
                ? 'Direct'
                : session.vpnStatus?.state === 'ready'
                  ? 'OK'
                  : session.vpnStatus?.state === 'unreachable'
                    ? 'Check'
                    : session.vpnStatus?.state === 'disconnected'
                      ? 'Down'
                      : 'VPN'}</span
          >
        </div>{/if}
      {#if connection.engine === 'redis'}
        <div class="schema-root">Database {connection.database} · Standalone</div>
      {:else}
        <div class="table-search">
          <Search size={13} /><input
            aria-label="Search tables"
            bind:value={session.search}
            placeholder="Find a table…"
            spellcheck="false"
            disabled={!session.online}
          />
        </div>
        <div class="table-list">
          {#each visibleTables as table}<button
              class:selected={page === 'explorer' &&
                workspace.selectedName === table.name &&
                workspace.schema === table.schema}
              onclick={() => selectTable(table)}
              title={table.readBlockedReason ?? table.relationKind ?? 'table'}
              disabled={busy || !session.online}><Table2 size={14} />{table.name}</button
            >{/each}
          {#if !visibleTables.length}<p class="sidebar-empty">
              {!session.online
                ? 'Reconnect to load tables'
                : search
                  ? 'No matching tables'
                  : 'No tables or views in this schema'}
            </p>{/if}
        </div>
      {/if}
    </div>
    <div class="sidebar-bottom">
      <div class="schema-control">
        <span>{connection.engine === 'postgres' ? 'Schema' : 'Database'}</span>
        {#if connection.engine === 'postgres' && !isSample}
          <button
            class="icon-button schema-refresh"
            aria-label="Refresh schema"
            title="Refresh schema"
            onclick={() => changeSchema(session.navSchema)}
            disabled={busy || !session.online}
          >
            <RefreshCw size={13} />
          </button>
        {/if}
        <select
          aria-label="Select PostgreSQL schema"
          value={session.navSchema}
          disabled={busy || !session.online || isSample || connection.engine !== 'postgres'}
          onchange={async (event) => {
            const select = event.currentTarget;
            await changeSchema(select.value);
            select.value = session.navSchema;
          }}
        >
          {#each [...new Set([session.navSchema, ...session.schemas])] as schema}
            <option value={schema}>{schema}</option>
          {/each}
        </select>
      </div>
      <button class:active={page === 'privacy'} onclick={() => navigate('privacy')} disabled={busy}
        ><Settings2 size={16} />Privacy & access<ChevronRight size={14} /></button
      >
      <div class="sidebar-version">
        <span>buggy-table</span><span class="version-actions"
          >v{__APP_VERSION__}<ThemeControl /></span
        >
      </div>
    </div>
  </aside>

  <div class="main-shell">
    <header class="topbar">
      <div class="breadcrumbs">
        <PanelLeft size={16} /><span
          title={`${connection.host}:${connection.port} / ${connection.database}`}
          >{isSample ? 'Workspace' : connection.name}</span
        ><ChevronRight size={12} /><strong
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
      {#if untestedPostgres}<div class="sample-banner" role="status">
          PostgreSQL {Math.floor(Number(connection.serverVersion) / 10000)} · Compatibility not yet tested.
          Tested versions: 15–18.
        </div>{/if}
      {#if error}<div class="error-banner global-error" role="alert">
          <ShieldCheck size={18} /><span>{error}</span><button
            class="icon-button"
            aria-label="Dismiss error"
            onclick={() => (error = '')}><X size={16} /></button
          >
        </div>{/if}

      {#if !session.online && page !== 'privacy'}
        <div class="feature-empty">
          <div class="large-icon"><Unplug size={28} /></div>
          <h2>{connection.name}</h2>
          <p>{engineName} · {connection.host}:{connection.port} / {connection.database}</p>
          <p>Saved on this device. Reconnect when you are ready.</p>
          <button class="primary-button" onclick={() => reconnect(session)} disabled={busy}
            ><PlugZap size={16} />{busy ? 'Connecting…' : 'Reconnect'}</button
          >
        </div>
      {:else if connection.engine === 'redis' && (page === 'explorer' || page === 'overview')}
        {#key `${connection.id}-${page}`}<RedisWorkspace
            {connection}
            bind:view={session.redis}
            showOverview={page === 'overview'}
            bind:busy
          />{/key}
      {:else if page === 'explorer'}
        {#key session.key}
          <TableTabs
            tabs={openTables}
            {activeId}
            {busy}
            onactivate={activateTab}
            onclose={closeTable}
            onreorder={(ids) => {
              if (busy || ids.length !== session.tabs.length) return;
              const byId = new Map(session.tabs.map((tab) => [tab.id, tab]));
              if (new Set(ids).size !== ids.length || ids.some((id) => !byId.has(id))) return;
              session.tabs = ids.map((id) => byId.get(id)!);
              persistLayout(session);
            }}
          />
        {/key}

        <div class="table-context">
          <h1>
            {workspace.selectedName || 'Query workspace'}<span class="object-tag"
              >{selected ? 'TABLE' : 'POSTGRESQL'}</span
            >
          </h1>
          <span class="muted small"
            >{connection.database} / {workspace.schema || session.navSchema}{#if selected}
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
          <button
            class="icon-button"
            aria-label={workspace.collapsed ? 'Show table controls' : 'Hide table controls'}
            aria-expanded={!workspace.collapsed}
            title={workspace.collapsed ? 'Show table controls' : 'Hide table controls'}
            onclick={() => (workspace.collapsed = !workspace.collapsed)}
            >{#if workspace.collapsed}<ChevronDown size={16} />{:else}<ChevronUp
                size={16}
              />{/if}</button
          >
          {#if workspace.collapsed && busy && activeRequest}<button
              class="secondary-button"
              onclick={cancel}>Cancel</button
            >{/if}
        </div>
        <section class="workbench-panel">
          {#if !workspace.collapsed}<div class="tabs" role="tablist" aria-label="Table views">
              <button
                role="tab"
                aria-selected={workspace.tab === 'data'}
                class:current={workspace.tab === 'data'}
                onclick={() => selectTab('data')}
                disabled={busy || !selected || !!selected.readBlockedReason}
                ><Table2 size={15} />Data</button
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
                disabled={busy || !selected || !!selected.readBlockedReason}
                ><ListTree size={15} />Indexes</button
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
            </div>{/if}

          {#if dirtyCount || workspace.uncertain || workspace.notice}
            <div class="save-bar" role="status">
              <span
                >{workspace.notice ||
                  (workspace.uncertain
                    ? 'Save outcome unknown · reload to reconcile'
                    : `${dirtyCount} row${dirtyCount === 1 ? '' : 's'} with unsaved changes${workspace.deletedRows.length ? ` · ${workspace.deletedRows.length} pending deletion` : ''}`)}</span
              >
              {#if dirtyCount || workspace.uncertain}<div>
                  {#if workspace.deletedRows.length}<button
                      class="secondary-button small-button"
                      disabled={busy}
                      onclick={() => setDeletedRows(workspace, [])}>Restore deleted rows</button
                    >{/if}
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
            {#if !workspace.collapsed}<div class="data-toolbar">
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
                <button
                  class="secondary-button"
                  class:filter-active={workspace.fullValues}
                  aria-pressed={workspace.fullValues}
                  title="Wrap complete cell values on this page"
                  onclick={() => {
                    workspace.fullValues = !workspace.fullValues;
                    workspace.scrollTop = 0;
                  }}><WrapText size={13} />Full values</button
                >
                <label class="inspector-toggle" title="Show read-only details when selecting a row">
                  <input
                    type="checkbox"
                    checked={inspector.enabled}
                    onchange={(event) => updateInspector({ enabled: event.currentTarget.checked })}
                  />Row details
                </label>
                {#if workspace.selectedRows.length}<button
                    class="secondary-button"
                    disabled={busy ||
                      !connection.manualEdits ||
                      (!selected?.deletable &&
                        workspace.selectedRows.some(
                          (i) => i < (workspace.result?.rows.length ?? 0),
                        ))}
                    onclick={() => stageDelete(workspace.selectedRows)}
                    ><Trash2 size={13} />Delete {workspace.selectedRows.length} selected</button
                  >{/if}
                <span class="tab-spacer"></span>
                {#if busy && activeRequest}<button class="secondary-button" onclick={cancel}
                    ><Square size={12} />Cancel</button
                  >{/if}
                <span class="muted small"
                  >{connection.manualEdits &&
                  selected?.columns.some((c) => c.editable) &&
                  selected?.columns.some((c) => c.primaryKey)
                    ? 'Select row, click again to edit · ⌘/Ctrl+S save'
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
                />{/if}{/if}
            {#key `${connection.id}:${workspace.id}`}<ResultGrid
                result={workspace.result}
                table={selected}
                {busy}
                editable={connection.manualEdits &&
                  !workspace.uncertain &&
                  !!selected?.columns.some((c) => c.primaryKey)}
                edits={workspace.edits}
                insertedRows={workspace.insertedRows}
                draftRevision={workspace.draftRevision}
                clipboardScope={`${connection.id}:${workspace.id}`}
                onpaste={pasteRows}
                oneditstart={(row, column) => beginCellEdit(workspace, row, column)}
                oneditend={() => endCellEdit(workspace)}
                onundo={(redo) => {
                  if (!busy) undoDraft(workspace, redo);
                }}
                deletedRows={workspace.deletedRows}
                bind:selectedRows={workspace.selectedRows}
                fullValues={workspace.fullValues}
                inspectorEnabled={inspector.enabled}
                inspectorWidth={inspector.widthPercent}
                oninspectorresize={(widthPercent) => updateInspector({ widthPercent })}
                rowOffset={workspace.pageNumber * workspace.pageSize}
                layoutKey={JSON.stringify([
                  connection.engine,
                  connection.host,
                  connection.port,
                  connection.database,
                  connection.username,
                  workspace.schema,
                  workspace.selectedName,
                ])}
                ondelete={stageDelete}
                onlayoutwarning={() =>
                  (error =
                    'Column order changed for this session, but local preferences could not be saved.')}
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
                  ? counting
                    ? 'Counting…'
                    : 'Reading…'
                  : `${workspace.result?.rows.length ?? 0} rows shown${workspace.insertedRows.length ? ` + ${workspace.insertedRows.length} new` : ''}`}{#if !isSample && workspace.result}<span
                    class="footer-separator">·</span
                  ><ResultTiming
                    result={workspace.result}
                  />{/if}{#if workspace.result?.truncated}<span class="footer-separator">·</span
                  >More rows or byte limit reached{/if}</span
              >
              <div
                class="row-total"
                title={workspace.countNotice ||
                  'Estimated from catalog statistics. Use Count exact for a fresh count.'}
              >
                <span
                  >{workspace.exactCount !== null
                    ? `${workspace.exactCount} ${workspace.appliedFilters.length ? 'matching' : 'total'} rows`
                    : isSample
                      ? `${workspace.result ? (sampleRows[selected?.name ?? '']?.length ?? 0) : 0} total rows`
                      : selected?.estimatedRows != null
                        ? `~${selected.estimatedRows.toLocaleString()} total rows`
                        : 'Total rows unknown'}</span
                >
                <button
                  class="secondary-button small-button"
                  disabled={busy || !selected}
                  onclick={countRows}>Count exact</button
                >
                {#if workspace.countNotice}<span class="count-notice" role="status"
                    >{workspace.countNotice}</span
                  >{/if}
              </div>
              <div class="pagination">
                <label
                  >Rows <select
                    aria-label="Rows per page"
                    value={workspace.pageSize}
                    disabled={busy || dirtyCount > 0 || workspace.uncertain}
                    onchange={async (event) => {
                      const input = event.currentTarget;
                      await changePageSize(Number(input.value));
                      input.value = String(workspace.pageSize);
                    }}
                  >
                    {#each [50, 100, 250, 500] as size}<option value={size}>{size}</option>{/each}
                  </select></label
                >
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
                    workspace.result.rows.length < workspace.pageSize}
                  onclick={() => loadData(selected, workspace.pageNumber + 1)}
                  ><ChevronRight size={16} /></button
                >
              </div>
            </div>
          {:else if workspace.tab === 'structure' && selected}
            <div class="section-intro">
              <h3>Table structure</h3>
              <p>{selected.relationKind ?? 'Table'} · Column definitions from this schema.</p>
              {#if selected.readBlockedReason}<p role="status">{selected.readBlockedReason}</p>{/if}
            </div>
            <div class="grid-scroll">
              <table class="structure-table">
                <thead
                  ><tr><th>Column</th><th>Data type</th><th>Nullable</th><th>Key</th></tr></thead
                ><tbody
                  >{#each selected.columns as column}<tr
                      ><td class="mono">{column.name}</td><td
                        ><span class="type-chip"
                          >{column.typeSchema && column.typeSchema !== 'pg_catalog'
                            ? `${column.typeSchema}.`
                            : ''}{column.dataType}</span
                        ></td
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
                  >{#if workspace.queryResult}{workspace.queryResult.rows.length} rows · <ResultTiming
                      result={workspace.queryResult}
                    />{workspace.queryResult.truncated
                      ? ' · Result limit reached'
                      : ''}{:else}Results appear here{/if}</span
                ><span>1,000 rows / 4 MiB maximum</span>
              </div>{/if}
          {/if}
        </section>
        <div class="workspace-footnote">
          <LockKeyhole size={12} />{isSample
            ? 'Sample data is generated in this app. No database is connected.'
            : `${connection.vpn?.enabled ? 'OpenVPN connection' : 'Direct connection'} to ${connection.host}:${connection.port} · ${connection.tls === 'verifyFull' ? 'Verified TLS' : connection.tls === 'require' ? 'TLS · certificate not verified' : 'Local loopback, unencrypted'} · ${session.saved?.hasPassword ? 'System password store' : 'Session-only password'}`}
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
          </div>{:else if overview}
          <div class="snapshot-charts" aria-label="Database snapshot graphs">
            <SnapshotChart
              label="Database size"
              value={overview.databaseSizeBytes ?? null}
              unit="bytes"
              points={metricPoints('databaseSizeBytes')}
            />
            <SnapshotChart
              label="Connections"
              value={overview.connections}
              unit="connections"
              points={metricPoints('connections')}
            />
            <SnapshotChart
              label="Active connections"
              value={overview.activeConnections}
              unit="connections"
              points={metricPoints('activeConnections')}
            />
            <SnapshotChart
              label="Buffer cache hit rate"
              value={overview.cacheHitPercent === null ? null : overview.cacheHitPercent.toFixed(2)}
              unit="%"
              ceiling={100}
              points={metricPoints('cacheHitPercent')}
            />
            <SnapshotChart
              label={connection.engine === 'mysql' ? 'COMMIT statements' : 'Committed transactions'}
              value={overview.transactionsCommitted}
              points={metricPoints('transactionsCommitted')}
            />
            <SnapshotChart
              label={connection.engine === 'mysql'
                ? 'ROLLBACK statements'
                : 'Rolled-back transactions'}
              value={overview.transactionsRolledBack}
              points={metricPoints('transactionsRolledBack')}
            />
          </div>
          <div class="detail-card">
            <h3>About this snapshot</h3>
            <dl>
              <dt>Source</dt>
              <dd>
                {connection.engine === 'mysql'
                  ? 'information_schema.TABLES · SHOW GLOBAL STATUS (server-wide)'
                  : 'pg_stat_database · pg_stat_activity'}
              </dd>
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
              allowlist. PostgreSQL manual cell saves use a separate, permission-checked UPDATE /
              DELETE operation with conflict detection. Row deletion stays pending until you save.
            </p>
            <span class="badge">No automatic writes or truncate tools</span>
          </article>
          <article class="detail-card">
            <Database size={22} />
            <h3>Direct database access</h3>
            <p>
              Database connections use the device network or their selected isolated OpenVPN
              profile. Signed releases can check GitHub for updates; database contents and
              credentials are never included. Turn off automatic checks in the update menu to stop
              background update requests. There are no analytics, cloud AI or remote assets.
            </p>
            <span class="badge">No database uploads</span>
          </article>
          <article class="detail-card">
            <ShieldCheck size={22} />
            <h3>Local connection storage</h3>
            <p>
              Active sessions, query text and results are held in memory. The app does not save
              query history or result caches to disk. Saved connection settings and table tab
              identities stay in the local app data folder; passwords are saved only when selected,
              in the operating system password store. Appearance and update preferences are stored
              locally, along with column order preferences for each table.
            </p>
            <span class="badge">No query/result persistence</span>
          </article>
          <article class="detail-card">
            <CircleHelp size={22} />
            <h3>Preview boundaries</h3>
            <p>
              PostgreSQL lists tables and views, including JSON, arrays and enums. Entries that
              cannot be read show the reason in Structure. MySQL and Redis expose supported reads;
              custom routines and advanced SQL remain unavailable.
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
                  : connection.tls === 'require'
                    ? 'Encrypted TLS, minimum TLS 1.2 · certificate not verified'
                    : 'Unencrypted · literal loopback IP only'}
            </dd>
            <dt>Reader</dt>
            <dd>{isSample ? 'Not applicable' : connection.username}</dd>
          </dl>
          {#if !isSample}<button class="secondary-button" onclick={disconnect} disabled={busy}
              ><Unplug size={15} />Disconnect this session</button
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
          : `${engineName} · ${session.online ? 'session ready' : 'disconnected'}`}</span
      ><span
        >{desktopAvailable ? 'Desktop' : 'Browser preview'}<span class="footer-separator">·</span
        >All results stay in this app</span
      >
    </footer>
  </div>
</div>
{#if showConnection}<ConnectionDialog
    onconnected={connected}
    initial={connectionInitial}
    onclose={() => (showConnection = false)}
  />{/if}

{#if showManager}<ConnectionManager
    {entries}
    {activeKey}
    {busy}
    onselect={switchConnection}
    onreconnect={reconnect}
    onadd={openConnection}
    onedit={editConnection}
    onpin={pinConnection}
    onremove={removeConnection}
    onclose={() => (showManager = false)}
    onvpn={openVpn}
    ontogglevpn={toggleVpn}
  />{/if}
{#if vpnOwner && vpnEntry}<VpnDialog
    owner={vpnOwner}
    name={vpnEntry.info.name}
    selection={vpnSelection(vpnEntry)}
    status={vpnEntry.vpnStatus}
    online={vpnEntry.online}
    onapply={(selection) => changeVpn(vpnEntry!, selection)}
    onclose={() => (vpnOwner = null)}
    ontest={() => testVpn(vpnEntry!)}
    onrefresh={() => refreshVpn(vpnEntry!)}
    onprofileupdate={(profile) => {
      if (vpnEntry) vpnEntry.saved = profile;
    }}
  />{/if}
<UpdateNotice
  blocked={busy || anyDirty || showConnection || showManager || !!vpnOwner}
  bind:installing={updating}
/>

<svelte:window onkeydown={handleShortcut} />
{#if closeTarget}<ConfirmClose
    name={openTables.find((t) => t.id === closeTarget)?.selectedName ?? 'table'}
    onsave={saveAndClose}
    ondiscard={() => closeTable(closeTarget!, true)}
    oncancel={() => (closeTarget = null)}
    {busy}
  />{/if}
