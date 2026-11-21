(() => {
  "use strict";
  const ACCEPTANCE_CASES = Object.freeze([
    "preflight",
    "native-smoke",
    "trust-reload",
    "turn-history",
    "approval",
    "terminal",
    "files-watch-plugins",
    "network-independent",
    "backpressure",
    "remote-recovery",
    "remote-peer",
    "measurements",
  ]);
  const REQUEST_PREFIX = "acceptance-req-";
  const responses = new Map();
  let requestCounter = 0;
  const callbacks = window.__TAURI_INTERNALS__?.callbacks;
  if (!callbacks || typeof callbacks.set !== "function" || typeof callbacks.get !== "function") {
    throw new Error("Pinned Tauri callback registry unavailable");
  }
  const originalSet = callbacks.set;
  const observed = new Map();
  const wrapped = new WeakSet();
  let gate = false;
  let held = null;
  // Sustained hold: the renderer consumes nothing until release, so native delivery stays unacked.
  let holdAll = false;
  const heldAll = [];
  let invalid = false;
  let bootstrap = null;
  let runPromise = null;

  function compact(event) {
    const result = {
      sessionId: event.sessionId,
      generation: event.generation,
      sequence: event.sequence,
      kind: event.kind,
      serverInfoReceived: false,
      requestCompleted: false,
    };
    if (typeof event.text === "string") {
      try {
        const packet = JSON.parse(event.text);
        const message = packet.message ?? packet;
        const payload = message.payload ?? message;
        // Only this scenario's own requests are retained; application traffic is never buffered.
        const responseId = message.requestId ?? message.id;
        if (typeof responseId === "string" && responseId.startsWith(REQUEST_PREFIX)) {
          result.response = {
            id: responseId,
            success: message.success !== false && !payload.error,
            payload,
            error: message.error ?? payload.error,
          };
        }
        result.serverInfoReceived = payload.status === "server_info";
        const request = payload.requestId ?? message.requestId ?? packet.requestId;
        result.requestCompleted =
          typeof request === "string" &&
          request.length > 0 &&
          payload.status !== "error" &&
          message.type !== "error" &&
          !payload.error &&
          payload.success !== false;
      } catch {
        /* Binary/non-JSON data is not an authenticated RPC observation. */
      }
    }
    return result;
  }

  function metadata(event) {
    if (event.kind !== "message") return;
    const key = `${event.sessionId}:${event.generation}`;
    if (!observed.has(key) && observed.size >= 128) {
      invalid = true;
      return;
    }
    const previous = observed.get(key) ?? {
      sessionId: event.sessionId,
      generation: event.generation,
      lastReceivedSequence: 0,
      serverInfoReceived: false,
      requestCompleted: false,
    };
    if (event.sequence <= previous.lastReceivedSequence) {
      invalid = true;
      return;
    }
    previous.lastReceivedSequence = event.sequence;
    previous.serverInfoReceived ||= event.serverInfoReceived;
    previous.requestCompleted ||= event.requestCompleted;
    observed.set(key, previous);
  }

  function wrap(callback) {
    if (typeof callback !== "function" || wrapped.has(callback)) return callback;
    let next = 0;
    const pending = new Map();
    function deliver(data, event) {
      const result = callback(data); // Preserve actual callback execution, return value and errors.
      if (event.response) {
        if (responses.size >= 32) responses.delete(responses.keys().next().value);
        responses.set(event.response.id, event.response);
      }
      if (!Number.isSafeInteger(data.index) || data.index < 0) {
        invalid = true;
        return result;
      }
      if (data.index === next) {
        metadata(event);
        next++;
        while (pending.has(next)) {
          metadata(pending.get(next));
          pending.delete(next++);
        }
      } else if (data.index > next && pending.size < 65) {
        // Tauri Channel buffers out-of-order indices; do not call that application delivery.
        pending.set(data.index, event);
      } else invalid = true;
      return result;
    }
    const wrapper = function (data) {
      const event = data?.message;
      if (
        !event ||
        typeof event.sessionId !== "string" ||
        typeof event.generation !== "string" ||
        !Number.isSafeInteger(event.sequence) ||
        event.sequence <= 0
      )
        return callback(data);
      const observedEvent = compact(event);
      if (holdAll && event.kind === "message") {
        heldAll.push(() => deliver(data, observedEvent));
        return;
      }
      if (gate && held === null && event.kind === "message") {
        gate = false;
        held = () => deliver(data, observedEvent); // Exactly one real callback is held; no synthetic ACK or event.
        return;
      }
      return deliver(data, observedEvent);
    };
    wrapped.add(wrapper);
    return wrapper;
  }

  for (const [id, callback] of callbacks) originalSet.call(callbacks, id, wrap(callback));
  // Tauri 2.12 makes invoke/runCallback immutable. Only decorate the existing Map's registration.
  Object.defineProperty(callbacks, "set", {
    value: function (id, callback) {
      return originalSet.call(this, id, this === callbacks ? wrap(callback) : callback);
    },
  });
  window.__KRONOS_IPC_ACCEPTANCE_METADATA__ = () =>
    invalid ? [] : [...observed.values()].map((entry) => Object.assign({}, entry));

  function release() {
    gate = false;
    holdAll = false;
    const queued = heldAll.splice(0);
    for (const deliverHeld of queued) {
      try {
        deliverHeld();
      } catch {
        /* A stale held callback must not abort releasing the rest. */
      }
    }
    const callback = held;
    held = null;
    callback?.();
  }

  window.__KRONOS_IPC_ACCEPTANCE_HOLD_ALL__ = (enabled) => {
    if (enabled) holdAll = true;
    else release();
  };

  window.__KRONOS_IPC_ACCEPTANCE_ACK_GATE__ = (enabled) => {
    if (enabled) {
      if (held) throw new Error("Only one ACK gate may be held");
      gate = true;
    } else {
      release();
    }
  };

  async function invokeWithin(command, input, deadline) {
    const remaining = deadline - window.performance.now();
    if (remaining <= 0) throw new Error("Acceptance scenario deadline expired");
    let timer;
    try {
      return await Promise.race([
        window.__TAURI_INTERNALS__.invoke(command, { input }),
        new Promise((_, reject) => {
          timer = window.setTimeout(
            () => reject(new Error("Acceptance scenario deadline expired")),
            remaining,
          );
        }),
      ]);
    } finally {
      window.clearTimeout(timer);
    }
  }

  function sidebarVisible() {
    // The project-list child does not exist in a fresh, empty owned home.
    const sidebar = window.document.querySelector(
      '[data-testid="sidebar-project-workspace-list-scroll"]',
    );
    if (!sidebar) return false;
    const style = window.getComputedStyle(sidebar);
    return (
      [...sidebar.getClientRects()].some((rect) => rect.width > 0 && rect.height > 0) &&
      style.visibility === "visible" &&
      style.display !== "none"
    );
  }

  function deliveredAndAcknowledged(native, received) {
    return (
      received?.serverInfoReceived &&
      received.requestCompleted &&
      Number.isSafeInteger(native.lastAckSequence) &&
      received.lastReceivedSequence > 0 &&
      received.lastReceivedSequence <= native.lastAckSequence &&
      native.lastAckSequence <= native.lastSentSequence &&
      native.closeCode == null
    );
  }

  function validBootstrap(command, keys) {
    const secondary = command.caseId === "trust-reload" && command.role === "secondary";
    const expectedKeys = secondary
      ? "caseId,primaryGeneration,role,runId,surfaceGeneration,type"
      : "caseId,role,runId,surfaceGeneration,type";
    if (
      keys !== expectedKeys ||
      typeof command.runId !== "string" ||
      !/^[a-f0-9]{32}$/.test(command.runId)
    )
      return false;
    if (!ACCEPTANCE_CASES.includes(command.caseId) || !["main", "secondary"].includes(command.role))
      return false;
    if (!Number.isSafeInteger(command.surfaceGeneration) || command.surfaceGeneration <= 0)
      return false;
    return (
      !secondary ||
      (Number.isSafeInteger(command.primaryGeneration) &&
        command.primaryGeneration > 0 &&
        command.primaryGeneration !== command.surfaceGeneration)
    );
  }

  function captureBootstrap(command, keys) {
    if (!validBootstrap(command, keys)) throw new Error("Invalid acceptance bootstrap handoff");
    if (bootstrap && Object.keys(bootstrap).some((key) => bootstrap[key] !== command[key]))
      throw new Error("Acceptance bootstrap replacement rejected");
    if (!bootstrap && command.caseId === "trust-reload") {
      const key = `__KRONOS_IPC_ACCEPTANCE_TRUST_RELOAD__:${command.runId}:${command.role}`;
      const previous = window.sessionStorage?.getItem(key);
      bootstrap = Object.freeze({
        ...command,
        previous: previous ? JSON.parse(previous) : null,
      });
      window.sessionStorage?.setItem(
        key,
        JSON.stringify({ generation: command.surfaceGeneration }),
      );
    } else bootstrap ??= Object.freeze({ ...command });
    return { ...bootstrap, bootstrapped: true };
  }

  // Source-verified reasons; a blocked case never reports pass and never fabricates observations.
  const BLOCKED_REASONS = Object.freeze({
    "turn-history":
      "harness-pending: no owned engine-restart control and no UI-driven prompt submission yet",
    approval:
      "native-unsupported: kernel/server.rs always reports pendingPermissions: [] (no provider permission request/response path)",
    terminal:
      "native-unsupported: native_pty.rs has no snapshot/replay API, so reconnect-without-gaps cannot be observed",
    "files-watch-plugins":
      "native-unsupported: NativePluginManager exposes only metrics/enabled toggles, no plugin traffic subscription path",
    "network-independent":
      "harness-pending: decoy bind and stopped-endpoint credential rejection assertions not implemented",
    "remote-recovery":
      "harness-pending: the driver has no second owned native peer/home supervisor path",
    "remote-peer":
      "harness-pending: the driver has no second owned native peer/home supervisor path",
    measurements: "harness-pending: alternating IPC/WebSocket workload driver not implemented",
  });

  async function reportBlocked(identity) {
    const reason = BLOCKED_REASONS[identity.caseId];
    if (!reason) throw new Error("Acceptance scenario not implemented for this case/role");
    if (identity.role !== "main") throw new Error("Blocked acceptance cases require the main role");
    const deadline = window.performance.now() + 120000;
    const scope = { runId: identity.runId, surfaceGeneration: identity.surfaceGeneration };
    const native = await waitBoundSession(identity, deadline);
    await invokeWithin(
      "ipc_acceptance_report",
      {
        ...scope,
        report: {
          outcome: "blocked",
          reason,
          visibleState: "not-exercised",
          sessionId: native.sessionId,
          generation: native.generation,
        },
      },
      deadline,
    );
    await invokeWithin("ipc_acceptance_control", { ...scope, action: { type: "exit" } }, deadline);
    return { blocked: true, engineEpoch: native.engineEpoch };
  }

  function assertNativeIdentity(native, identity) {
    if (
      native.runId !== identity.runId ||
      native.caseId !== identity.caseId ||
      native.role !== identity.role ||
      native.incarnation !== identity.surfaceGeneration
    )
      throw new Error("Acceptance scenario native identity mismatch");
  }

  async function waitBoundSession(identity, deadline) {
    while (window.performance.now() < deadline) {
      try {
        const native = await invokeWithin(
          "ipc_acceptance_control",
          {
            runId: identity.runId,
            surfaceGeneration: identity.surfaceGeneration,
            action: { type: "observe" },
          },
          Math.min(deadline, window.performance.now() + 5000),
        );
        assertNativeIdentity(native, identity);
        if (
          native.sessionId &&
          native.generation &&
          native.closeCode === null &&
          Number.isSafeInteger(native.engineEpoch) &&
          native.engineEpoch > 0
        )
          return native;
      } catch (error) {
        if (
          !["No owned acceptance session", "Acceptance session has no bound core"].includes(
            String(error?.message ?? error),
          )
        )
          throw error;
      }
      await new Promise((resolve) => window.setTimeout(resolve, 50));
    }
    throw new Error("Acceptance scenario deadline expired before session binding");
  }

  async function waitDelivery(identity, deadline, requiredEpoch) {
    while (window.performance.now() < deadline) {
      const native = await waitBoundSession(identity, deadline);
      if (requiredEpoch !== undefined && native.engineEpoch !== requiredEpoch)
        throw new Error("Trust-reload changed the embedded core epoch");
      const received = window
        .__KRONOS_IPC_ACCEPTANCE_METADATA__()
        .find(
          (entry) => entry.sessionId === native.sessionId && entry.generation === native.generation,
        );
      if (native.opened && sidebarVisible() && deliveredAndAcknowledged(native, received))
        return { native, received };
      await new Promise((resolve) => window.setTimeout(resolve, 50));
    }
    throw new Error("Acceptance scenario deadline expired");
  }

  async function nativeSmoke(identity) {
    if (identity.caseId !== "native-smoke" || identity.role !== "main")
      throw new Error("Acceptance scenario not implemented for this case/role");
    const deadline = window.performance.now() + 120000;
    const scope = { runId: identity.runId, surfaceGeneration: identity.surfaceGeneration };
    while (window.performance.now() < deadline) {
      let native;
      try {
        native = await invokeWithin(
          "ipc_acceptance_control",
          { ...scope, action: { type: "observe" } },
          deadline,
        );
      } catch (error) {
        // Only these two observations are transient during normal frontend/core startup.
        if (
          !["No owned acceptance session", "Acceptance session has no bound core"].includes(
            String(error?.message ?? error),
          )
        )
          throw error;
      }
      if (native) {
        if (
          native.runId !== identity.runId ||
          native.caseId !== identity.caseId ||
          native.role !== identity.role ||
          native.incarnation !== identity.surfaceGeneration
        )
          throw new Error("Acceptance scenario native identity mismatch");
        const received = window
          .__KRONOS_IPC_ACCEPTANCE_METADATA__()
          .find(
            (entry) =>
              entry.sessionId === native.sessionId && entry.generation === native.generation,
          );
        if (sidebarVisible() && deliveredAndAcknowledged(native, received)) {
          await invokeWithin(
            "ipc_acceptance_report",
            {
              ...scope,
              report: {
                outcome: "pass",
                visibleState: "sidebar-visible",
                sessionId: received.sessionId,
                generation: received.generation,
                lastReceivedSequence: received.lastReceivedSequence,
                serverInfoReceived: true,
                requestCompleted: true,
              },
            },
            deadline,
          );
          await invokeWithin(
            "ipc_acceptance_control",
            { ...scope, action: { type: "exit" } },
            deadline,
          );
          return { reported: true };
        }
      }
      await new Promise((resolve) => window.setTimeout(resolve, 50));
    }
    throw new Error("Acceptance scenario deadline expired");
  }

  async function requireGenerationRejection(identity, generation, deadline, label) {
    try {
      await invokeWithin(
        "ipc_acceptance_control",
        {
          runId: identity.runId,
          surfaceGeneration: generation,
          action: { type: "open-gate", enabled: false },
        },
        deadline,
      );
    } catch (error) {
      if (String(error?.message ?? error).includes("generation mismatch")) return true;
      throw error;
    }
    throw new Error(`Trust-reload ${label} generation was accepted`);
  }

  async function trustReloadSecondary(identity, deadline, scope) {
    await requireGenerationRejection(identity, identity.primaryGeneration, deadline, "cross-owner");
    await invokeWithin(
      "ipc_acceptance_control",
      { ...scope, action: { type: "open-gate", enabled: false } },
      deadline,
    );
    const { native, received } = await waitDelivery(identity, deadline);
    await invokeWithin(
      "ipc_acceptance_report",
      {
        ...scope,
        report: {
          outcome: "pass",
          visibleState: "secondary-sidebar-visible",
          sessionId: received.sessionId,
          generation: received.generation,
          lastReceivedSequence: received.lastReceivedSequence,
          serverInfoReceived: true,
          requestCompleted: true,
          crossOwnerRejected: true,
        },
      },
      deadline,
    );
    await invokeWithin(
      "ipc_acceptance_control",
      {
        ...scope,
        action: { type: "reload", role: "main" },
      },
      deadline,
    );
    return { reloadedMain: true, engineEpoch: native.engineEpoch };
  }

  async function trustReloadInitialMain(identity, deadline, scope, storageKey) {
    const pending = await waitBoundSession(identity, deadline);
    if (pending.opened !== false)
      throw new Error("Trust-reload main open was not held before reload");
    const guest = await invokeWithin(
      "ipc_acceptance_control",
      {
        ...scope,
        action: { type: "create", role: "guest" },
      },
      deadline,
    );
    if (guest.guestUnregistered !== true)
      throw new Error("Trust-reload guest acquired native authority");
    window.sessionStorage?.setItem(
      storageKey,
      JSON.stringify({
        generation: identity.surfaceGeneration,
        engineEpoch: pending.engineEpoch,
        sessionId: pending.sessionId,
        guestRejected: true,
      }),
    );
    await invokeWithin(
      "ipc_acceptance_control",
      {
        ...scope,
        action: { type: "create", role: "secondary" },
      },
      deadline,
    );
    while (window.performance.now() < deadline) {
      try {
        await invokeWithin(
          "ipc_acceptance_control",
          { ...scope, action: { type: "observe" } },
          deadline,
        );
      } catch (error) {
        if (/stale|unregistered|generation mismatch/i.test(String(error?.message ?? error)))
          return { awaitingSuccessor: true };
        throw error;
      }
      await new Promise((resolve) => window.setTimeout(resolve, 50));
    }
    throw new Error("Trust-reload secondary did not reload the held main surface");
  }

  async function trustReloadSuccessor(identity, deadline, scope, storageKey, previous) {
    if (
      !Number.isSafeInteger(previous.generation) ||
      !Number.isSafeInteger(previous.engineEpoch) ||
      previous.generation === identity.surfaceGeneration ||
      previous.guestRejected !== true
    )
      throw new Error("Trust-reload prior main identity is invalid");
    await requireGenerationRejection(identity, previous.generation, deadline, "stale");
    await invokeWithin(
      "ipc_acceptance_control",
      { ...scope, action: { type: "open-gate", enabled: false } },
      deadline,
    );
    const { native, received } = await waitDelivery(identity, deadline, previous.engineEpoch);
    if (native.sessionId === previous.sessionId || native.generation === previous.generation)
      throw new Error("Trust-reload successor reused the detached session");
    window.sessionStorage?.setItem(
      storageKey,
      JSON.stringify({
        generation: identity.surfaceGeneration,
        engineEpoch: native.engineEpoch,
        sessionId: native.sessionId,
        guestRejected: true,
      }),
    );
    await invokeWithin(
      "ipc_acceptance_report",
      {
        ...scope,
        report: {
          outcome: "pass",
          visibleState: "sidebar-visible",
          sessionId: received.sessionId,
          generation: received.generation,
          lastReceivedSequence: received.lastReceivedSequence,
          serverInfoReceived: true,
          requestCompleted: true,
          staleGenerationRejected: true,
          sameEngineEpoch: native.engineEpoch === previous.engineEpoch,
          guestRejected: previous.guestRejected,
        },
      },
      deadline,
    );
    await invokeWithin("ipc_acceptance_control", { ...scope, action: { type: "exit" } }, deadline);
    return { reported: true };
  }

  async function trustReload(identity) {
    const deadline = window.performance.now() + 120000;
    const scope = { runId: identity.runId, surfaceGeneration: identity.surfaceGeneration };
    if (identity.role === "secondary") return trustReloadSecondary(identity, deadline, scope);
    if (identity.role !== "main")
      throw new Error("Trust-reload requires a trusted main or secondary surface");
    const storageKey = `__KRONOS_IPC_ACCEPTANCE_TRUST_RELOAD__:${identity.runId}:main`;
    return identity.previous
      ? trustReloadSuccessor(identity, deadline, scope, storageKey, identity.previous)
      : trustReloadInitialMain(identity, deadline, scope, storageKey);
  }

  // Envelope methods route on `id`; session methods route on `requestId`. Both are real paths.
  async function sendFrame(native, type, payload, deadline, envelope = false) {
    const id = `${REQUEST_PREFIX}${++requestCounter}`;
    const frame = envelope ? { id, type, payload } : { requestId: id, type, payload };
    await invokeWithin(
      "send_embedded_transport",
      {
        sessionId: native.sessionId,
        generation: native.generation,
        text: JSON.stringify(frame),
      },
      deadline,
    );
    return id;
  }

  async function sendRequest(native, type, payload, deadline, envelope = false) {
    const id = await sendFrame(native, type, payload, deadline, envelope);
    while (window.performance.now() < deadline) {
      const response = responses.get(id);
      if (response) {
        responses.delete(id);
        if (!response.success)
          throw new Error(`Acceptance request ${type} failed: ${response.error ?? "unknown"}`);
        return response.payload;
      }
      await new Promise((resolve) => window.setTimeout(resolve, 20));
    }
    throw new Error(`Acceptance request ${type} deadline expired`);
  }

  function observe(deadline) {
    return invokeWithin(
      "ipc_acceptance_control",
      {
        runId: bootstrap.runId,
        surfaceGeneration: bootstrap.surfaceGeneration,
        action: { type: "observe" },
      },
      deadline,
    );
  }

  // After a real close the client resynchronizes onto a new live session; the closed one stays in
  // bounded native history, so each close is attributable to its own generation.
  async function waitLiveSession(deadline, previous) {
    while (window.performance.now() < deadline) {
      let native;
      try {
        native = await observe(Math.min(deadline, window.performance.now() + 5000));
      } catch {
        await new Promise((resolve) => window.setTimeout(resolve, 50));
        continue;
      }
      if (
        native.sessionId !== previous.sessionId &&
        native.opened &&
        native.closeCode === null &&
        Number.isSafeInteger(native.engineEpoch) &&
        native.engineEpoch > 0
      )
        return native;
      await new Promise((resolve) => window.setTimeout(resolve, 50));
    }
    throw new Error("Acceptance session did not resynchronize after close");
  }

  async function waitClose(deadline, sessionId, code) {
    let nextReport = 0;
    while (window.performance.now() < deadline) {
      let native;
      let failure;
      try {
        native = await observe(Math.min(deadline, window.performance.now() + 5000));
      } catch (error) {
        failure = String(error?.message ?? error);
      }
      if (native) {
        const closed = (native.closedSessions ?? []).find((entry) => entry.sessionId === sessionId);
        if (closed?.closeCode === code) return closed;
      }
      if (window.performance.now() >= nextReport) {
        nextReport = window.performance.now() + 5000;
        step(
          failure
            ? `wait-${code}-error-${failure.slice(0, 40)}`
            : `wait-${code}-live-${sid(native)}-closed-${(native.closedSessions ?? [])
                .map((entry) => entry.closeCode)
                .join(",")}`,
        );
      }
      await new Promise((resolve) => window.setTimeout(resolve, 50));
    }
    throw new Error(`Acceptance close ${code} was never recorded natively`);
  }

  async function ownedWorkspace(native, deadline) {
    const listed = await sendRequest(native, "fetch_workspaces_request", {}, deadline);
    const owned = (listed.entries ?? []).find((entry) =>
      String(entry.workspaceDirectory ?? "")
        .split("\\")
        .join("/")
        .endsWith(`/kronos-ipc-acceptance-${native.runId}/workspace`),
    );
    if (owned) return owned.workspaceDirectory;
    // A fresh owned home has no workspace yet; register the app's own run-owned working directory.
    const created = await sendRequest(
      native,
      "workspace.create.request",
      { source: { path: "." } },
      deadline,
    );
    const directory =
      created.workspace?.workspaceDirectory ?? created.workspaceDirectory ?? created.path;
    const normalized = String(directory ?? "")
      .split("\\")
      .join("/");
    if (!normalized.endsWith(`/kronos-ipc-acceptance-${native.runId}/workspace`))
      throw new Error(
        `Workspace creation did not stay run-owned: ${JSON.stringify(created).slice(0, 300)}`,
      );
    return directory;
  }

  // Diagnostic breadcrumbs: the webview's persistent storage survives a run that never reaches its
  // report, so the last step reached stays readable in the retained profile. Never certified evidence.
  const breadcrumbs = [];
  function step(name) {
    breadcrumbs.push(`${Math.round(window.performance.now())}:${name}`);
    while (breadcrumbs.length > 80) breadcrumbs.shift();
    try {
      window.localStorage?.setItem("kronos-acceptance-breadcrumbs", breadcrumbs.join("|"));
    } catch {
      /* Breadcrumbs are diagnostics only. */
    }
  }
  const sid = (native) => String(native?.sessionId ?? "?").slice(-6);

  async function backpressure(identity) {
    const deadline = window.performance.now() + 300000;
    const scope = { runId: identity.runId, surfaceGeneration: identity.surfaceGeneration };
    const control = (action) =>
      invokeWithin("ipc_acceptance_control", { ...scope, action }, deadline);
    const initial = await waitBoundSession(identity, deadline);
    step("bound");
    const cwd = await ownedWorkspace(initial, deadline);
    step("workspace");
    const created = await sendRequest(
      initial,
      "agent/create",
      { provider: "codex", cwd, title: "Acceptance backpressure" },
      deadline,
    );
    const agentId = created.payload?.id ?? created.id ?? created.agent?.id;
    if (typeof agentId !== "string" || !agentId)
      throw new Error("Controlled agent creation returned no id");
    step("agent");
    // Stop real consumption before the one controlled turn starts, so native in-flight frames
    // actually accumulate instead of being acknowledged as they arrive.
    window.__KRONOS_IPC_ACCEPTANCE_HOLD_ALL__(true);
    await sendFrame(initial, "agent/message", { id: agentId, text: "Acceptance burst" }, deadline);
    step("message-sent");
    const overflow = await waitClose(deadline, initial.sessionId, 1013);
    step(`close-1013-peak-${overflow.channelPeakEvents}`);
    window.__KRONOS_IPC_ACCEPTANCE_HOLD_ALL__(false);
    step("released");
    const resynced = await waitLiveSession(deadline, initial);
    step(`resynced-1013-${sid(resynced)}`);
    await sendRequest(resynced, "daemon/get_status", {}, deadline);
    const timeline = await sendRequest(resynced, "agent/timeline", { id: agentId }, deadline);
    const burstItems = (timeline.items ?? []).filter(
      (item) => JSON.stringify(item).length >= 4096,
    ).length;
    step(`burst-${burstItems}`);
    if (burstItems < 200)
      throw new Error(`Burst delivery incomplete after resync: ${burstItems} items`);

    // Oversized frame rejection is a separate real close, not the overflow close.
    await invokeWithin(
      "send_embedded_transport",
      {
        sessionId: resynced.sessionId,
        generation: resynced.generation,
        text: "x".repeat(16 * 1024 * 1024 + 1),
      },
      deadline,
    ).catch(() => undefined);
    const oversized = await waitClose(deadline, resynced.sessionId, 1009);
    step("close-1009");
    const afterOversized = await waitLiveSession(deadline, resynced);
    step(`resynced-1009-${sid(afterOversized)}`);

    // A real writer fault is its own recoverable close.
    await control({ type: "writer-gate", enabled: true });
    step(`writer-gate-armed-${sid(afterOversized)}`);
    try {
      await sendFrame(afterOversized, "daemon/get_status", {}, deadline);
      step("status-sent");
    } catch (error) {
      step(`status-send-failed:${String(error?.message ?? error).slice(0, 40)}`);
    }
    const writerFault = await waitClose(deadline, afterOversized.sessionId, 1011);
    await control({ type: "writer-gate", enabled: false });
    step("close-1011");
    const afterWriter = await waitLiveSession(deadline, afterOversized);
    step("resynced-1011");

    // Ordinary client close.
    await invokeWithin(
      "close_embedded_transport",
      { sessionId: afterWriter.sessionId, generation: afterWriter.generation },
      deadline,
    ).catch(() => undefined);
    const ordinary = await waitClose(deadline, afterWriter.sessionId, 1000);
    step("close-1000");
    const final = await waitLiveSession(deadline, afterWriter);
    step("resynced-1000");
    await sendRequest(final, "daemon/get_status", {}, deadline);
    const received = window
      .__KRONOS_IPC_ACCEPTANCE_METADATA__()
      .find(
        (entry) => entry.sessionId === final.sessionId && entry.generation === final.generation,
      );
    if (!received?.serverInfoReceived || !sidebarVisible())
      throw new Error("Resynchronized session did not restore the frontend");
    for (const [label, entry] of [
      ["overflow", overflow],
      ["oversized", oversized],
      ["writer", writerFault],
      ["ordinary", ordinary],
    ]) {
      if (!entry) throw new Error(`Missing ${label} close evidence`);
    }
    await invokeWithin(
      "ipc_acceptance_report",
      {
        ...scope,
        report: {
          outcome: "pass",
          visibleState: "burst-drained",
          sessionId: final.sessionId,
          generation: final.generation,
          lastReceivedSequence: received.lastReceivedSequence,
          serverInfoReceived: true,
          requestCompleted: true,
          burstDelivered: true,
          resyncObserved: true,
        },
      },
      deadline,
    );
    await control({ type: "exit" });
    return { reported: true };
  }

  function selectScenario(identity) {
    if (identity.caseId === "trust-reload") return trustReload(identity);
    if (identity.caseId === "native-smoke") return nativeSmoke(identity);
    if (identity.caseId === "backpressure") return backpressure(identity);
    return reportBlocked(identity);
  }

  // Fixed entry: the only stable interface for native scenario control. Commands are a
  // closed set, and a failed command must never strand the one held callback.
  window.__KRONOS_IPC_ACCEPTANCE_START__ = async (command) => {
    try {
      if (!command || typeof command !== "object" || Array.isArray(command)) {
        throw new Error("Acceptance scenario command must be an object");
      }
      const keys = Object.keys(command).sort().join(",");
      if (command.type === "bootstrap") return captureBootstrap(command, keys);
      if (command.type === "ack-gate") {
        if (keys !== "enabled,type" || typeof command.enabled !== "boolean") {
          throw new Error("Acceptance ACK gate requires a boolean enabled field");
        }
        window.__KRONOS_IPC_ACCEPTANCE_ACK_GATE__(command.enabled);
        return { applied: true };
      }
      if (keys !== "type") {
        throw new Error("Acceptance scenario command has unexpected fields");
      }
      if (command.type === "run") {
        if (!bootstrap) throw new Error("Acceptance scenario requires native bootstrap");
        step("run");
        runPromise ??= selectScenario(bootstrap)
          .catch(async (error) => {
            step(`fail:${String(error?.message ?? error).slice(0, 96)}`);
            // A failed scenario must record its own reason and end its run, never idle to the deadline.
            const bounded = (promise) =>
              Promise.race([
                promise,
                new Promise((_, reject) =>
                  window.setTimeout(() => reject(new Error("Failure report bound")), 2000),
                ),
              ]);
            try {
              const failureScope = await bounded(
                window.__TAURI_INTERNALS__.invoke("ipc_acceptance_control", {
                  input: {
                    runId: bootstrap.runId,
                    surfaceGeneration: bootstrap.surfaceGeneration,
                    action: { type: "observe" },
                  },
                }),
              );
              await bounded(
                window.__TAURI_INTERNALS__.invoke("ipc_acceptance_report", {
                  input: {
                    runId: bootstrap.runId,
                    surfaceGeneration: bootstrap.surfaceGeneration,
                    report: {
                      outcome: "fail",
                      reason: String(error?.message ?? error).slice(0, 512),
                      sessionId: failureScope.sessionId,
                      generation: failureScope.generation,
                    },
                  },
                }),
              );
            } catch {
              /* Reporting a failure is best-effort; the original error is still rethrown. */
            }
            try {
              await window.__TAURI_INTERNALS__.invoke("ipc_acceptance_control", {
                input: {
                  runId: bootstrap.runId,
                  surfaceGeneration: bootstrap.surfaceGeneration,
                  action: { type: "exit" },
                },
              });
            } catch {
              /* Exiting a failed run is best-effort; the original error is rethrown. */
            }
            throw error;
          })
          .finally(release);
        return await runPromise;
      }
      if (command.type === "metadata") return window.__KRONOS_IPC_ACCEPTANCE_METADATA__();
      if (command.type === "release") {
        release();
        return { released: true };
      }
      throw new Error("Unknown acceptance scenario command");
    } catch (error) {
      release(); // No rejected command leaves the held callback dangling.
      throw error;
    }
  };
})();
