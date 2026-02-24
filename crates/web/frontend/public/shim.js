// IPC Shim Layer for Xenobot - Adapts Electron IPC calls to HTTP/WebSocket
// This file is injected into the frontend to provide compatibility with the Rust backend.

(function() {
    'use strict';
    
    console.log('Xenobot IPC shim loaded');
    
    // Helper function to make HTTP requests
    async function httpRequest(method, path, data = null) {
        const baseUrl = '/api'; // API base path
        const url = `${baseUrl}${path}`;
        const options = {
            method,
            headers: {
                'Content-Type': 'application/json',
            },
        };
        if (data) {
            options.body = JSON.stringify(data);
        }
        try {
            const response = await fetch(url, options);
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }
            return await response.json();
        } catch (error) {
            console.error(`HTTP request failed: ${method} ${path}`, error);
            throw error;
        }
    }
    
    function createNotImplementedAdapter(namespace) {
        return new Proxy({}, {
            get(target, method) {
                return async (...args) => {
                    console.log(`[IPC Shim] ${namespace}.${method} called with args:`, args);
                    throw new Error(`IPC shim not implemented: ${namespace}.${method}`);
                };
            }
        });
    }

    function toQuery(params) {
        const query = new URLSearchParams();
        Object.entries(params).forEach(([key, value]) => {
            if (value === undefined || value === null) {
                return;
            }
            if (Array.isArray(value)) {
                value.forEach((item) => {
                    if (item !== undefined && item !== null) {
                        query.append(key, String(item));
                    }
                });
                return;
            }
            query.set(key, String(value));
        });
        const text = query.toString();
        return text ? `?${text}` : '';
    }

    function toNumber(value, fallback = 0) {
        const n = Number(value);
        return Number.isFinite(n) ? n : fallback;
    }

    function snakeToCamel(key) {
        return String(key).replace(/_([a-z])/g, (_, c) => c.toUpperCase());
    }

    function toCamelDeep(value) {
        if (Array.isArray(value)) {
            return value.map((item) => toCamelDeep(item));
        }
        if (value && typeof value === 'object') {
            const out = {};
            Object.entries(value).forEach(([k, v]) => {
                out[snakeToCamel(k)] = toCamelDeep(v);
            });
            return out;
        }
        return value;
    }

    function parseAliases(value) {
        if (Array.isArray(value)) {
            return value.filter((x) => typeof x === 'string');
        }
        if (typeof value === 'string') {
            try {
                const parsed = JSON.parse(value);
                if (Array.isArray(parsed)) {
                    return parsed.filter((x) => typeof x === 'string');
                }
            } catch (_) {
                return [];
            }
        }
        return [];
    }

    function normalizeSession(raw) {
        const item = toCamelDeep(raw || {});
        return {
            id: String(item.id ?? ''),
            name: item.name ?? '',
            platform: item.platform ?? 'unknown',
            type: item.type ?? item.chatType ?? 'group',
            importedAt: toNumber(item.importedAt, 0),
            messageCount: toNumber(item.messageCount, 0),
            memberCount: toNumber(item.memberCount, 0),
            dbPath: item.dbPath ?? '',
            groupId: item.groupId ?? null,
            groupAvatar: item.groupAvatar ?? null,
            ownerId: item.ownerId ?? null,
            memberAvatar: item.memberAvatar ?? null,
            summaryCount: toNumber(item.summaryCount, 0),
            aiConversationCount: toNumber(item.aiConversationCount, 0),
        };
    }

    function normalizeMember(raw, messageCountMap) {
        const item = toCamelDeep(raw || {});
        const id = toNumber(item.id, 0);
        return {
            id,
            platformId: item.platformId ?? '',
            accountName: item.accountName ?? null,
            groupNickname: item.groupNickname ?? null,
            aliases: parseAliases(item.aliases),
            messageCount: toNumber(item.messageCount, messageCountMap?.get(id) ?? 0),
            avatar: item.avatar ?? null,
        };
    }

    function toDateStringFromPeriod(period) {
        const text = String(toNumber(period, 0));
        if (/^\d{8}$/.test(text)) {
            return `${text.slice(0, 4)}-${text.slice(4, 6)}-${text.slice(6, 8)}`;
        }
        const ts = toNumber(period, 0);
        if (ts > 0) {
            const d = new Date(ts * 1000);
            if (!Number.isNaN(d.getTime())) {
                return d.toISOString().slice(0, 10);
            }
        }
        return '';
    }

    function normalizeTimeFilter(filter) {
        if (!filter) {
            return {};
        }
        const f = toCamelDeep(filter);
        return {
            start_ts: f.startTs ?? null,
            end_ts: f.endTs ?? null,
        };
    }

    function normalizeImportProgressStatus(status) {
        const s = String(status || '').toLowerCase();
        if (s === 'detecting') return 'detecting';
        if (s === 'reading') return 'reading';
        if (s === 'parsing') return 'parsing';
        if (s === 'saving') return 'saving';
        if (s === 'completed' || s === 'done' || s === 'success' || s === 'idle') return 'done';
        if (s === 'failed' || s === 'error') return 'error';
        return 'reading';
    }

    function normalizeImportProgressSnapshot(snapshot) {
        const item = toCamelDeep(snapshot || {});
        const total = toNumber(item.total, 0);
        const processed = toNumber(item.processed, 0);
        const stage = normalizeImportProgressStatus(item.status);
        const progress = total > 0 ? Math.max(0, Math.min(100, Math.round((processed / total) * 100))) : (stage === 'done' ? 100 : 0);
        return {
            stage,
            progress,
            message: item.error || item.currentFile || '',
            total,
            processed,
        };
    }

    async function readImportProgressSnapshot() {
        const response = await fetch('/api/chat/import-progress', { method: 'GET' });
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        const text = await response.text();
        const dataLine = text
            .split('\n')
            .filter((line) => line.startsWith('data:'))
            .map((line) => line.slice(5).trim())
            .join('\n');
        if (!dataLine) {
            return null;
        }
        try {
            return JSON.parse(dataLine);
        } catch (_) {
            return null;
        }
    }

    function normalizeMentionAnalysis(raw) {
        const input = toCamelDeep(raw || {});
        const topMentioners = (input.topMentioners || []).map((m) => ({
            memberId: toNumber(m.memberId, 0),
            platformId: m.platformId ?? '',
            name: m.name ?? '',
            count: toNumber(m.count ?? m.mentionerCount, 0),
            percentage: toNumber(m.percentage ?? m.mentionRate, 0),
        }));
        const topMentioned = (input.topMentioned || []).map((m) => ({
            memberId: toNumber(m.memberId, 0),
            platformId: m.platformId ?? '',
            name: m.name ?? '',
            count: toNumber(m.count ?? m.mentionedCount, 0),
            percentage: toNumber(m.percentage ?? m.mentionRate, 0),
        }));

        const oneWay = (input.oneWay || []).map((x) => ({
            fromMemberId: toNumber(x.fromMemberId, 0),
            fromName: x.fromName ?? '',
            toMemberId: toNumber(x.toMemberId, 0),
            toName: x.toName ?? '',
            fromToCount: toNumber(x.fromToCount ?? x.count, 0),
            toFromCount: toNumber(x.toFromCount, 0),
            ratio: toNumber(x.ratio, 0),
        }));

        const twoWay = (input.twoWay || []).map((x) => ({
            member1Id: toNumber(x.member1Id ?? x.memberAId, 0),
            member1Name: x.member1Name ?? x.memberAName ?? '',
            member2Id: toNumber(x.member2Id ?? x.memberBId, 0),
            member2Name: x.member2Name ?? x.memberBName ?? '',
            member1To2: toNumber(x.member1To2 ?? x.countAb, 0),
            member2To1: toNumber(x.member2To1 ?? x.countBa, 0),
            total: toNumber(x.total, 0),
            balance: toNumber(x.balance ?? x.ratio, 0),
        }));

        const detailMap = new Map();
        const ensureDetail = (memberId, name) => {
            if (!detailMap.has(memberId)) {
                detailMap.set(memberId, {
                    memberId,
                    name: name || `Member ${memberId}`,
                    topMentioned: [],
                    topMentioners: [],
                });
            }
            return detailMap.get(memberId);
        };

        oneWay.forEach((pair) => {
            const from = ensureDetail(pair.fromMemberId, pair.fromName);
            const to = ensureDetail(pair.toMemberId, pair.toName);
            from.topMentioned.push({
                fromMemberId: pair.fromMemberId,
                fromName: pair.fromName,
                toMemberId: pair.toMemberId,
                toName: pair.toName,
                count: pair.fromToCount,
            });
            to.topMentioners.push({
                fromMemberId: pair.fromMemberId,
                fromName: pair.fromName,
                toMemberId: pair.toMemberId,
                toName: pair.toName,
                count: pair.fromToCount,
            });
        });

        twoWay.forEach((pair) => {
            const m1 = ensureDetail(pair.member1Id, pair.member1Name);
            const m2 = ensureDetail(pair.member2Id, pair.member2Name);
            m1.topMentioned.push({
                fromMemberId: pair.member1Id,
                fromName: pair.member1Name,
                toMemberId: pair.member2Id,
                toName: pair.member2Name,
                count: pair.member1To2,
            });
            m1.topMentioners.push({
                fromMemberId: pair.member2Id,
                fromName: pair.member2Name,
                toMemberId: pair.member1Id,
                toName: pair.member1Name,
                count: pair.member2To1,
            });
            m2.topMentioned.push({
                fromMemberId: pair.member2Id,
                fromName: pair.member2Name,
                toMemberId: pair.member1Id,
                toName: pair.member1Name,
                count: pair.member2To1,
            });
            m2.topMentioners.push({
                fromMemberId: pair.member1Id,
                fromName: pair.member1Name,
                toMemberId: pair.member2Id,
                toName: pair.member2Name,
                count: pair.member1To2,
            });
        });

        topMentioners.forEach((m) => ensureDetail(m.memberId, m.name));
        topMentioned.forEach((m) => ensureDetail(m.memberId, m.name));

        const memberDetails = Array.from(detailMap.values()).map((detail) => ({
            ...detail,
            topMentioned: detail.topMentioned.sort((a, b) => b.count - a.count).slice(0, 10),
            topMentioners: detail.topMentioners.sort((a, b) => b.count - a.count).slice(0, 10),
        }));

        const totalMentions = toNumber(
            input.totalMentions,
            twoWay.reduce((sum, p) => sum + p.total, 0) + oneWay.reduce((sum, p) => sum + p.fromToCount, 0),
        );

        return {
            topMentioners,
            topMentioned,
            oneWay,
            twoWay,
            totalMentions,
            memberDetails,
        };
    }

    // Session API mapping
    window.sessionApi = {
        generate: (sessionId, gapThreshold) => {
            const body = {};
            if (gapThreshold !== undefined) body.gapThreshold = gapThreshold;
            return httpRequest('POST', `/session/generate/${encodeURIComponent(sessionId)}`, body);
        },
        hasIndex: (sessionId) =>
            httpRequest('GET', `/session/has-index/${encodeURIComponent(sessionId)}`),
        getStats: (sessionId) =>
            httpRequest('GET', `/session/stats/${encodeURIComponent(sessionId)}`),
        clear: (sessionId) =>
            httpRequest('POST', `/session/clear/${encodeURIComponent(sessionId)}`, {}),
        updateGapThreshold: (sessionId, gapThreshold) =>
            httpRequest('POST', `/session/update-gap-threshold/${encodeURIComponent(sessionId)}`, { gapThreshold }),
        getSessions: (sessionId) =>
            httpRequest('GET', `/session/sessions/${encodeURIComponent(sessionId)}`),
        generateSummary: (dbSessionId, chatSessionId, locale, forceRegenerate = false) =>
            httpRequest(
                'POST',
                `/session/generate-summary/${encodeURIComponent(dbSessionId)}/${encodeURIComponent(chatSessionId)}`,
                { locale, forceRegenerate }
            ),
        generateSummaries: (dbSessionId, chatSessionIds, locale) =>
            httpRequest(
                'POST',
                `/session/generate-summaries/${encodeURIComponent(dbSessionId)}`,
                { chatSessionIds, locale }
            ),
        checkCanGenerateSummary: (dbSessionId, chatSessionIds) =>
            httpRequest(
                'POST',
                `/session/check-can-generate-summary/${encodeURIComponent(dbSessionId)}`,
                { chatSessionIds }
            ),
        getByTimeRange: (dbSessionId, startTs, endTs) =>
            httpRequest(
                'GET',
                `/session/by-time-range/${encodeURIComponent(dbSessionId)}${toQuery({ startTs, endTs })}`
            ),
        getRecent: (dbSessionId, limit) =>
            httpRequest(
                'GET',
                `/session/recent/${encodeURIComponent(dbSessionId)}${toQuery({ limit })}`
            ),
    };

    // Merge API mapping
    window.mergeApi = {
        parseFileInfo: (filePath) =>
            httpRequest('POST', '/merge/parse-file-info', { file_path: filePath }),
        checkConflicts: (filePaths) =>
            httpRequest('POST', '/merge/check-conflicts', { file_paths: filePaths }),
        mergeFiles: (params) =>
            httpRequest('POST', '/merge/merge-files', params),
        clearCache: (filePath) =>
            httpRequest('POST', '/merge/clear-cache', { file_path: filePath }),
    };

    // Chat API mapping
    window.chatApi = {
        checkMigration: async () => {
            const result = await httpRequest('GET', '/chat/check-migration');
            return {
                needsMigration: !!result.needsMigration,
                count: toNumber(result.count, 0),
                currentVersion: toNumber(result.currentVersion, 0),
                pendingMigrations: Array.isArray(result.pendingMigrations) ? result.pendingMigrations : [],
            };
        },
        runMigration: () => httpRequest('POST', '/chat/run-migration', {}),
        selectFile: async () => ({ error: 'error.no_file_selected' }),
        import: (filePath) => httpRequest('POST', '/chat/import', { file_path: filePath }),
        detectFormat: (filePath) => httpRequest('POST', '/chat/detect-format', { file_path: filePath }),
        importWithOptions: (filePath, formatOptions = {}) =>
            httpRequest('POST', '/chat/import-with-options', {
                file_path: filePath,
                format_options: formatOptions,
            }),
        scanMultiChatFile: (filePath) => httpRequest('POST', '/chat/scan-multi-chat-file', { file_path: filePath }),
        onImportProgress: (callback) => {
            let active = true;
            let timer = null;

            const poll = async () => {
                if (!active) return;
                try {
                    const snapshot = await readImportProgressSnapshot();
                    const normalized = normalizeImportProgressSnapshot(snapshot || {});
                    callback(normalized);
                    if (normalized.stage === 'done' || normalized.stage === 'error') {
                        return;
                    }
                } catch (_) {
                    // Ignore polling errors and continue.
                }
                timer = setTimeout(poll, 500);
            };

            poll();
            return () => {
                active = false;
                if (timer) {
                    clearTimeout(timer);
                }
            };
        },

        getSessions: async () => {
            const list = await httpRequest('GET', '/chat/sessions');
            return (Array.isArray(list) ? list : []).map((item) => normalizeSession(item));
        },
        getSession: async (sessionId) => {
            const item = await httpRequest('GET', `/chat/sessions/${encodeURIComponent(sessionId)}`);
            return normalizeSession(item);
        },
        deleteSession: (sessionId) => httpRequest('DELETE', `/chat/sessions/${encodeURIComponent(sessionId)}`),
        renameSession: (sessionId, newName) =>
            httpRequest('POST', `/chat/sessions/${encodeURIComponent(sessionId)}/rename`, { new_name: newName }),
        updateSessionOwnerId: (sessionId, ownerId) =>
            httpRequest('POST', `/chat/sessions/${encodeURIComponent(sessionId)}/owner`, { owner_id: ownerId }),

        getAvailableYears: (sessionId, filter) =>
            httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/available-years${toQuery(normalizeTimeFilter(filter))}`
            ),
        getTimeRange: async (sessionId) => {
            const result = await httpRequest('GET', `/chat/sessions/${encodeURIComponent(sessionId)}/time-range`);
            const item = toCamelDeep(result || {});
            const start = item.start ?? item.earliest ?? null;
            const end = item.end ?? item.latest ?? null;
            if (start === null || end === null) {
                return null;
            }
            return { start: toNumber(start, 0), end: toNumber(end, 0) };
        },
        getMemberActivity: async (sessionId, filter) => {
            const result = await httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/member-activity${toQuery(normalizeTimeFilter(filter))}`
            );
            return (Array.isArray(result) ? result : []).map((item) => {
                const m = toCamelDeep(item);
                return {
                    memberId: toNumber(m.memberId, 0),
                    platformId: m.platformId ?? '',
                    name: m.name ?? '',
                    avatar: m.avatar ?? null,
                    messageCount: toNumber(m.messageCount, 0),
                    percentage: toNumber(m.percentage, 0),
                };
            });
        },
        getHourlyActivity: async (sessionId, filter) => {
            const result = await httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/hourly-activity${toQuery(normalizeTimeFilter(filter))}`
            );
            return (Array.isArray(result) ? result : []).map((item) => {
                const x = toCamelDeep(item);
                return { hour: toNumber(x.period ?? x.hour, 0), messageCount: toNumber(x.messageCount, 0) };
            });
        },
        getDailyActivity: async (sessionId, filter) => {
            const result = await httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/daily-activity${toQuery(normalizeTimeFilter(filter))}`
            );
            return (Array.isArray(result) ? result : []).map((item) => {
                const x = toCamelDeep(item);
                return {
                    date: toDateStringFromPeriod(x.period ?? x.date),
                    messageCount: toNumber(x.messageCount, 0),
                };
            });
        },
        getWeekdayActivity: async (sessionId, filter) => {
            const result = await httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/weekday-activity${toQuery(normalizeTimeFilter(filter))}`
            );
            return (Array.isArray(result) ? result : []).map((item) => {
                const x = toCamelDeep(item);
                return {
                    weekday: toNumber(x.period ?? x.weekday, 0),
                    messageCount: toNumber(x.messageCount, 0),
                };
            });
        },
        getMonthlyActivity: async (sessionId, filter) => {
            const result = await httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/monthly-activity${toQuery(normalizeTimeFilter(filter))}`
            );
            return (Array.isArray(result) ? result : []).map((item) => {
                const x = toCamelDeep(item);
                return {
                    month: toNumber(x.period ?? x.month, 0),
                    messageCount: toNumber(x.messageCount, 0),
                };
            });
        },
        getYearlyActivity: async (sessionId, filter) => {
            const result = await httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/yearly-activity${toQuery(normalizeTimeFilter(filter))}`
            );
            return (Array.isArray(result) ? result : []).map((item) => {
                const x = toCamelDeep(item);
                return {
                    year: toNumber(x.period ?? x.year, 0),
                    messageCount: toNumber(x.messageCount, 0),
                };
            });
        },
        getMessageLengthDistribution: (sessionId, filter) =>
            httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/message-length-distribution${toQuery(normalizeTimeFilter(filter))}`
            ),
        getMessageTypeDistribution: async (sessionId, filter) => {
            const result = await httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/message-type-distribution${toQuery(normalizeTimeFilter(filter))}`
            );
            return (Array.isArray(result) ? result : []).map((item) => {
                const x = toCamelDeep(item);
                return {
                    type: toNumber(x.type ?? x.msgType, 0),
                    count: toNumber(x.count, 0),
                };
            });
        },

        getCatchphraseAnalysis: async (sessionId, filter) => {
            const result = await httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/catchphrase-analysis${toQuery(normalizeTimeFilter(filter))}`
            );
            const data = toCamelDeep(result || {});
            return {
                members: (data.members || []).map((m) => ({
                    memberId: toNumber(m.memberId, 0),
                    platformId: m.platformId ?? '',
                    name: m.name ?? '',
                    catchphrases: Array.isArray(m.catchphrases) ? m.catchphrases : [],
                })),
            };
        },
        getMentionAnalysis: async (sessionId, filter) => {
            const result = await httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/mention-analysis${toQuery(normalizeTimeFilter(filter))}`
            );
            return normalizeMentionAnalysis(result);
        },
        getMentionGraph: (sessionId, filter) =>
            httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/mention-graph${toQuery(normalizeTimeFilter(filter))}`
            ),
        getClusterGraph: (sessionId, filter) =>
            httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/cluster-graph${toQuery(normalizeTimeFilter(filter))}`
            ),
        getLaughAnalysis: (sessionId, filter, keywords = []) => {
            const query = {
                ...normalizeTimeFilter(filter),
                keywords,
            };
            return httpRequest('GET', `/chat/sessions/${encodeURIComponent(sessionId)}/laugh-analysis${toQuery(query)}`);
        },

        getMembers: async (sessionId) => {
            const [membersRaw, activityRaw] = await Promise.all([
                httpRequest('GET', `/chat/sessions/${encodeURIComponent(sessionId)}/members`),
                httpRequest('GET', `/chat/sessions/${encodeURIComponent(sessionId)}/member-activity`).catch(() => []),
            ]);
            const activityMap = new Map(
                (Array.isArray(activityRaw) ? activityRaw : []).map((item) => {
                    const x = toCamelDeep(item);
                    return [toNumber(x.memberId, 0), toNumber(x.messageCount, 0)];
                })
            );
            return (Array.isArray(membersRaw) ? membersRaw : []).map((item) => normalizeMember(item, activityMap));
        },
        getMembersPaginated: async (sessionId, params = {}) => {
            const query = {
                page: params.page,
                page_size: params.pageSize ?? params.page_size,
                search: params.search,
                sort_order: params.sortOrder ?? params.sort_order,
            };
            const result = await httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/members-paginated${toQuery(query)}`
            );
            const data = toCamelDeep(result || {});
            return {
                members: (data.members || []).map((item) => normalizeMember(item)),
                total: toNumber(data.total, 0),
                page: toNumber(data.page, 1),
                pageSize: toNumber(data.pageSize, 20),
                totalPages: toNumber(data.totalPages, 0),
            };
        },
        getMemberNameHistory: async (sessionId, memberId) => {
            const result = await httpRequest(
                'GET',
                `/chat/sessions/${encodeURIComponent(sessionId)}/member-name-history/${encodeURIComponent(memberId)}`
            );
            return (Array.isArray(result) ? result : []).map((item) => {
                const x = toCamelDeep(item);
                return {
                    nameType: x.nameType ?? '',
                    name: x.name ?? '',
                    startTs: toNumber(x.startTs, 0),
                    endTs: x.endTs === null || x.endTs === undefined ? null : toNumber(x.endTs, 0),
                };
            });
        },
        updateMemberAliases: (sessionId, memberId, aliases) =>
            httpRequest(
                'POST',
                `/chat/sessions/${encodeURIComponent(sessionId)}/members/${encodeURIComponent(memberId)}/aliases`,
                { aliases }
            ),
        deleteMember: (sessionId, memberId) =>
            httpRequest('DELETE', `/chat/sessions/${encodeURIComponent(sessionId)}/members/${encodeURIComponent(memberId)}`),

        pluginQuery: (sessionId, sql, params) =>
            httpRequest('POST', `/chat/sessions/${encodeURIComponent(sessionId)}/plugin-query`, {
                sql,
                params,
            }),
        pluginCompute: (fnString, input) =>
            httpRequest('POST', '/chat/plugin-compute', {
                fn_string: fnString,
                input,
            }),
        executeSQL: (sessionId, sql) =>
            httpRequest('POST', `/chat/sessions/${encodeURIComponent(sessionId)}/execute-sql`, { sql }),
        getSchema: (sessionId) =>
            httpRequest('GET', `/chat/sessions/${encodeURIComponent(sessionId)}/schema`),

        analyzeIncrementalImport: (sessionId, filePath) =>
            httpRequest(
                'POST',
                `/chat/sessions/${encodeURIComponent(sessionId)}/analyze-incremental-import`,
                { file_path: filePath }
            ),
        incrementalImport: (sessionId, filePath) =>
            httpRequest(
                'POST',
                `/chat/sessions/${encodeURIComponent(sessionId)}/incremental-import`,
                { file_path: filePath }
            ),

        exportSessionsToTempFiles: (sessionIds) =>
            httpRequest('POST', '/chat/export-sessions-to-temp-files', {
                session_ids: (sessionIds || []).map((id) => String(id)),
            }),
        cleanupTempExportFiles: (filePaths) =>
            httpRequest('POST', '/chat/cleanup-temp-export-files', {
                file_paths: filePaths || [],
            }),
        getDbDirectory: () => httpRequest('GET', '/chat/db-directory'),
        getSupportedFormats: () => httpRequest('GET', '/chat/supported-formats'),
    };

    function randomId(prefix) {
        return `${prefix}_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
    }

    function normalizeAiMessage(raw) {
        const item = toCamelDeep(raw || {});
        return {
            id: toNumber(item.id, 0),
            senderName: item.senderName ?? '',
            senderPlatformId: item.senderPlatformId ?? '',
            senderAliases: parseAliases(item.senderAliases),
            senderAvatar: item.senderAvatar ?? null,
            content: item.content ?? '',
            timestamp: toNumber(item.timestamp, 0),
            type: toNumber(item.type ?? item.msgType, 0),
            replyToMessageId: item.replyToMessageId ?? null,
            replyToContent: item.replyToContent ?? null,
            replyToSenderName: item.replyToSenderName ?? null,
            isHit: Boolean(item.isHit),
        };
    }

    function normalizeFilterResult(raw) {
        const data = toCamelDeep(raw || {});
        const blocks = (Array.isArray(data.blocks) ? data.blocks : []).map((block) => ({
            startTs: toNumber(block.startTs, 0),
            endTs: toNumber(block.endTs, 0),
            hitCount: toNumber(block.hitCount, 0),
            messages: (Array.isArray(block.messages) ? block.messages : []).map(normalizeAiMessage),
        }));
        return {
            blocks,
            stats: {
                totalMessages: toNumber(data.stats?.totalMessages, 0),
                hitMessages: toNumber(data.stats?.hitMessages, 0),
                totalChars: toNumber(data.stats?.totalChars, 0),
            },
            pagination: {
                page: toNumber(data.pagination?.page, 1),
                pageSize: toNumber(data.pagination?.pageSize, 50),
                totalBlocks: toNumber(data.pagination?.totalBlocks, blocks.length),
                totalHits: toNumber(data.pagination?.totalHits, 0),
                hasMore: Boolean(data.pagination?.hasMore),
            },
        };
    }

    async function consumeSseResponse(response, onChunk) {
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        if (!response.body || !response.body.getReader) {
            return [];
        }

        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        const chunks = [];
        let buffer = '';

        const emitBlock = (block) => {
            if (!block) return;
            const lines = block.split('\n');
            const dataText = lines
                .filter((line) => line.startsWith('data:'))
                .map((line) => line.slice(5).trimStart())
                .join('\n');
            if (!dataText) return;

            let parsed;
            try {
                parsed = JSON.parse(dataText);
            } catch (_) {
                parsed = { content: dataText };
            }
            const normalized = toCamelDeep(parsed);
            chunks.push(normalized);
            if (typeof onChunk === 'function') {
                try {
                    onChunk(normalized);
                } catch (err) {
                    console.error('SSE chunk callback failed', err);
                }
            }
        };

        while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            buffer += decoder.decode(value, { stream: true }).replace(/\r\n/g, '\n');
            let splitIndex = -1;
            while ((splitIndex = buffer.indexOf('\n\n')) !== -1) {
                const block = buffer.slice(0, splitIndex).trim();
                buffer = buffer.slice(splitIndex + 2);
                emitBlock(block);
            }
        }

        buffer += decoder.decode().replace(/\r\n/g, '\n');
        emitBlock(buffer.trim());
        return chunks;
    }

    async function postSse(path, payload, onChunk, signal) {
        const response = await fetch(`/api${path}`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                Accept: 'text/event-stream',
            },
            body: JSON.stringify(payload || {}),
            signal,
        });
        return consumeSseResponse(response, onChunk);
    }

    function subscribeSse(path, onChunk) {
        const controller = new AbortController();
        (async () => {
            try {
                const response = await fetch(`/api${path}`, {
                    method: 'GET',
                    headers: {
                        Accept: 'text/event-stream',
                    },
                    signal: controller.signal,
                });
                await consumeSseResponse(response, onChunk);
            } catch (error) {
                if (error && error.name === 'AbortError') {
                    return;
                }
                console.error(`SSE subscribe failed: ${path}`, error);
            }
        })();
        return () => controller.abort();
    }

    const activeAgentRequests = new Map();

    window.llmApi = {
        getProviders: () => httpRequest('GET', '/llm/providers'),
        getAllConfigs: () => httpRequest('GET', '/llm/configs'),
        getActiveConfigId: () => httpRequest('GET', '/llm/active-config-id'),
        addConfig: (config) => httpRequest('POST', '/llm/configs', config),
        updateConfig: (id, updates) =>
            httpRequest('POST', `/llm/configs/${encodeURIComponent(id)}`, updates || {}),
        deleteConfig: (id) => httpRequest('DELETE', `/llm/configs/${encodeURIComponent(id)}`),
        setActiveConfig: (id) => httpRequest('POST', '/llm/active-config', { id }),
        validateApiKey: (provider, apiKey, baseUrl, model) =>
            httpRequest('POST', '/llm/validate-api-key', {
                provider,
                apiKey,
                baseUrl,
                model,
            }),
        hasConfig: () => httpRequest('GET', '/llm/has-config'),
        chat: (messages, options) => httpRequest('POST', '/llm/chat', { messages, options }),
        chatStream: async (messages, options, onChunk) => {
            try {
                const chunks = await postSse('/llm/chat-stream', { messages, options }, (chunk) => {
                    if (typeof onChunk === 'function') {
                        onChunk(chunk);
                    }
                });
                const content = chunks.map((chunk) => chunk.content || '').join('');
                return {
                    success: true,
                    content,
                };
            } catch (error) {
                return {
                    success: false,
                    error: error instanceof Error ? error.message : String(error),
                };
            }
        },
    };

    window.embeddingApi = {
        getAllConfigs: () => httpRequest('GET', '/embedding/configs'),
        getConfig: (id) => httpRequest('GET', `/embedding/configs/${encodeURIComponent(id)}`),
        getActiveConfigId: () => httpRequest('GET', '/embedding/active-config-id'),
        isEnabled: () => httpRequest('GET', '/embedding/is-enabled'),
        addConfig: (config) => httpRequest('POST', '/embedding/configs', config),
        updateConfig: (id, updates) =>
            httpRequest('POST', `/embedding/configs/${encodeURIComponent(id)}`, updates || {}),
        deleteConfig: (id) => httpRequest('DELETE', `/embedding/configs/${encodeURIComponent(id)}`),
        setActiveConfig: (id) => httpRequest('POST', '/embedding/active-config', { id }),
        validateConfig: (config) => httpRequest('POST', '/embedding/validate-config', config),
        getVectorStoreStats: () => httpRequest('GET', '/embedding/vector-store-stats'),
        clearVectorStore: () => httpRequest('POST', '/embedding/clear-vector-store'),
    };

    window.aiApi = {
        searchMessages: async (sessionId, keywords = [], filter, limit = 100, offset = 0, senderId) => {
            const result = await httpRequest('POST', '/ai/search-messages', {
                sessionId,
                keywords: Array.isArray(keywords) ? keywords : [],
                filter,
                limit,
                offset,
                senderId,
            });
            const data = toCamelDeep(result || {});
            return {
                messages: (Array.isArray(data.messages) ? data.messages : []).map(normalizeAiMessage),
                count: toNumber(data.count, 0),
            };
        },
        semanticSearchMessages: async (sessionId, query, filter, threshold = 0.7, limit = 20) => {
            const result = await httpRequest('POST', '/ai/semantic-search-messages', {
                sessionId,
                query,
                filter,
                threshold,
                limit,
            });
            const data = toCamelDeep(result || {});
            const rows = Array.isArray(data.messages) ? data.messages : [];
            return {
                messages: rows.map((item) => {
                    const x = toCamelDeep(item || {});
                    const msg = normalizeAiMessage(x.message || {});
                    return {
                        ...msg,
                        similarity: Number.isFinite(Number(x.similarity)) ? Number(x.similarity) : 0,
                    };
                }),
                count: toNumber(data.count, 0),
                threshold: Number.isFinite(Number(data.threshold)) ? Number(data.threshold) : threshold,
                queryRewritten: data.queryRewritten || query || '',
            };
        },
        getMessageContext: async (sessionId, messageIdOrIds, contextSize = 3) => {
            const messageIds = Array.isArray(messageIdOrIds) ? messageIdOrIds : [messageIdOrIds];
            const result = await httpRequest('POST', '/ai/message-context', {
                sessionId,
                messageIds: messageIds
                    .map((id) => toNumber(id, 0))
                    .filter((id) => id > 0),
                contextSize,
            });
            return (Array.isArray(result) ? result : []).map(normalizeAiMessage);
        },
        getRecentMessages: async (sessionId, filter, limit = 50) => {
            const result = await httpRequest('POST', '/ai/recent-messages', {
                sessionId,
                filter,
                limit,
            });
            const data = toCamelDeep(result || {});
            return {
                messages: (Array.isArray(data.messages) ? data.messages : []).map(normalizeAiMessage),
                count: toNumber(data.count, 0),
            };
        },
        getAllRecentMessages: async (sessionId, filter, limit = 100) => {
            const result = await httpRequest('POST', '/ai/all-recent-messages', {
                sessionId,
                filter,
                limit,
            });
            const data = toCamelDeep(result || {});
            return {
                messages: (Array.isArray(data.messages) ? data.messages : []).map(normalizeAiMessage),
                count: toNumber(data.count, 0),
            };
        },
        getConversationBetween: async (sessionId, memberId1, memberId2, filter, limit = 200) => {
            const result = await httpRequest('POST', '/ai/conversation-between', {
                sessionId,
                memberId1,
                memberId2,
                filter,
                limit,
            });
            const data = toCamelDeep(result || {});
            return {
                messages: (Array.isArray(data.messages) ? data.messages : []).map(normalizeAiMessage),
                count: toNumber(data.count, 0),
            };
        },
        getMessagesBefore: async (sessionId, beforeId, limit = 50, filter, senderId, keywords) => {
            const result = await httpRequest('POST', '/ai/messages-before', {
                sessionId,
                beforeId,
                limit,
                filter,
                senderId,
                keywords,
            });
            const data = toCamelDeep(result || {});
            return {
                messages: (Array.isArray(data.messages) ? data.messages : []).map(normalizeAiMessage),
                hasMore: Boolean(data.hasMore),
            };
        },
        getMessagesAfter: async (sessionId, afterId, limit = 50, filter, senderId, keywords) => {
            const result = await httpRequest('POST', '/ai/messages-after', {
                sessionId,
                afterId,
                limit,
                filter,
                senderId,
                keywords,
            });
            const data = toCamelDeep(result || {});
            return {
                messages: (Array.isArray(data.messages) ? data.messages : []).map(normalizeAiMessage),
                hasMore: Boolean(data.hasMore),
            };
        },
        filterMessagesWithContext: async (
            sessionId,
            keywords,
            timeFilter,
            senderIds,
            contextSize = 10,
            page = 1,
            pageSize = 50
        ) => {
            const result = await httpRequest('POST', '/ai/filter-messages-with-context', {
                sessionId,
                keywords,
                timeFilter,
                senderIds,
                contextSize,
                page,
                pageSize,
            });
            return normalizeFilterResult(result);
        },
        getMultipleSessionsMessages: async (sessionId, chatSessionIds, page = 1, pageSize = 50) => {
            const result = await httpRequest('POST', '/ai/multiple-sessions-messages', {
                sessionId,
                chatSessionIds: Array.isArray(chatSessionIds) ? chatSessionIds : [],
                page,
                pageSize,
            });
            return normalizeFilterResult(result);
        },
        exportFilterResultToFile: (params) =>
            httpRequest('POST', '/ai/export-filter-result-to-file', params || {}),
        onExportProgress: (onProgress) =>
            subscribeSse('/ai/export-progress', (chunk) => {
                if (typeof onProgress === 'function') {
                    onProgress(chunk);
                }
            }),
        createConversation: (sessionId, title) =>
            httpRequest('POST', '/ai/conversations', {
                sessionId,
                title,
            }),
        getConversations: (sessionId) =>
            httpRequest('GET', `/ai/conversations${toQuery({ sessionId })}`),
        getConversation: (conversationId) =>
            httpRequest('GET', `/ai/conversations/${encodeURIComponent(conversationId)}`),
        updateConversationTitle: (conversationId, title) =>
            httpRequest('POST', `/ai/conversations/${encodeURIComponent(conversationId)}/title`, { title }),
        deleteConversation: (conversationId) =>
            httpRequest('DELETE', `/ai/conversations/${encodeURIComponent(conversationId)}`),
        addMessage: (
            conversationId,
            role,
            content,
            dataKeywords,
            dataMessageCount,
            contentBlocks
        ) =>
            httpRequest('POST', `/ai/conversations/${encodeURIComponent(conversationId)}/messages`, {
                role,
                content,
                dataKeywords,
                dataMessageCount,
                contentBlocks,
            }),
        getMessages: (conversationId) =>
            httpRequest('GET', `/ai/conversations/${encodeURIComponent(conversationId)}/messages`),
        deleteMessage: (messageId) =>
            httpRequest('DELETE', `/ai/messages/${encodeURIComponent(messageId)}`),
        showAiLogFile: () => httpRequest('GET', '/ai/show-ai-log-file'),
    };

    window.cacheApi = {
        getInfo: () => httpRequest('GET', '/cache/info'),
        clear: (cacheId) => httpRequest('POST', `/cache/clear/${encodeURIComponent(cacheId)}`),
        openDir: (cacheId) => httpRequest('POST', `/cache/open-dir/${encodeURIComponent(cacheId)}`),
        saveToDownloads: (filename, dataUrl) =>
            httpRequest('POST', '/cache/save-to-downloads', {
                filename,
                dataUrl,
            }),
        getLatestImportLog: () => httpRequest('GET', '/cache/latest-import-log'),
        getDataDir: () => httpRequest('GET', '/cache/data-dir'),
        selectDataDir: () => httpRequest('POST', '/cache/select-data-dir'),
        setDataDir: (path, migrate = true) =>
            httpRequest('POST', '/cache/set-data-dir', {
                path,
                migrate,
            }),
        showInFolder: (filePath) =>
            httpRequest('POST', '/cache/show-in-folder', {
                filePath,
            }),
    };

    window.networkApi = {
        getProxyConfig: () => httpRequest('GET', '/network/proxy-config'),
        saveProxyConfig: (config) => httpRequest('POST', '/network/proxy-config', config || {}),
        testProxyConnection: (proxyUrl) =>
            httpRequest('POST', '/network/test-proxy-connection', {
                proxyUrl,
            }),
    };

    window.nlpApi = {
        getWordFrequency: (params) => httpRequest('POST', '/nlp/word-frequency', params || {}),
        segmentText: (text, locale = 'zh-CN', minLength) =>
            httpRequest('POST', '/nlp/segment-text', {
                text,
                locale,
                minLength,
            }),
        getPosTags: async () => {
            const result = await httpRequest('GET', '/nlp/pos-tags');
            return (Array.isArray(result) ? result : []).map((item) => {
                const x = toCamelDeep(item);
                return {
                    tag: x.id ?? x.tag ?? '',
                    name: x.nameCn ?? x.nameEn ?? x.name ?? x.id ?? '',
                    description: x.description ?? '',
                    meaningful: Boolean(x.meaningful),
                };
            });
        },
    };

    window.agentApi = {
        runStream: (userMessage, context = {}, onChunk, historyMessages = [], chatType, promptConfig, locale) => {
            const requestId = randomId('agent');
            const controller = new AbortController();
            activeAgentRequests.set(requestId, controller);

            const ownerInfo = context.ownerInfo || context.owner_info;
            const payload = {
                user_message: userMessage,
                context: {
                    session_id: context.sessionId ?? context.session_id ?? '',
                    time_filter: context.timeFilter
                        ? {
                              start_ts: context.timeFilter.startTs ?? context.timeFilter.start_ts ?? null,
                              end_ts: context.timeFilter.endTs ?? context.timeFilter.end_ts ?? null,
                          }
                        : context.time_filter,
                    max_messages_limit: context.maxMessagesLimit ?? context.max_messages_limit ?? null,
                    owner_info: ownerInfo
                        ? {
                              id: toNumber(ownerInfo.id ?? ownerInfo.platformId, 0),
                              name: ownerInfo.name ?? ownerInfo.displayName ?? '',
                              avatar_url: ownerInfo.avatarUrl ?? ownerInfo.avatar_url ?? null,
                          }
                        : null,
                    locale: context.locale ?? locale ?? null,
                },
                history_messages: Array.isArray(historyMessages) ? historyMessages : [],
                chat_type: chatType || 'group',
                prompt_config: promptConfig
                    ? {
                          role_definition: promptConfig.roleDefinition ?? promptConfig.role_definition ?? '',
                          response_rules: promptConfig.responseRules ?? promptConfig.response_rules ?? '',
                      }
                    : null,
                locale: locale || context.locale || 'zh-CN',
            };

            const promise = (async () => {
                const toolsUsed = new Set();
                let content = '';
                let usage = null;
                try {
                    const chunks = await postSse(
                        '/agent/run-stream',
                        payload,
                        (chunk) => {
                            if (chunk.type === 'content' && chunk.content) {
                                content += chunk.content;
                            }
                            if (chunk.type === 'tool_start' && chunk.toolName) {
                                toolsUsed.add(chunk.toolName);
                            }
                            if (chunk.type === 'done' && chunk.usage) {
                                usage = chunk.usage;
                            }
                            if (typeof onChunk === 'function') {
                                onChunk(chunk);
                            }
                        },
                        controller.signal
                    );

                    for (const chunk of chunks) {
                        if (chunk.type === 'tool_start' && chunk.toolName) {
                            toolsUsed.add(chunk.toolName);
                        }
                    }

                    return {
                        success: true,
                        result: {
                            content,
                            toolsUsed: Array.from(toolsUsed),
                            toolRounds: toolsUsed.size,
                            totalUsage: usage,
                        },
                    };
                } catch (error) {
                    if (error && error.name === 'AbortError') {
                        return {
                            success: false,
                            error: 'aborted',
                        };
                    }
                    return {
                        success: false,
                        error: error instanceof Error ? error.message : String(error),
                    };
                } finally {
                    activeAgentRequests.delete(requestId);
                }
            })();

            return { requestId, promise };
        },
        abort: async (requestId) => {
            const controller = activeAgentRequests.get(requestId);
            if (controller) {
                controller.abort();
                activeAgentRequests.delete(requestId);
            }
            return httpRequest('POST', `/agent/abort/${encodeURIComponent(requestId)}`);
        },
    };
    
    // window.api.* (app-level APIs)
    window.api = {
        send: (channel, ...args) => {
            console.log(`[API stub] send('${channel}')`, args);
            return null;
        },
        app: {
            getVersion: () => httpRequest('GET', '/core/app/version'),
            getAnalyticsEnabled: () => httpRequest('GET', '/core/app/analytics-enabled'),
            setAnalyticsEnabled: (enabled) => httpRequest('POST', '/core/app/analytics-enabled', { enabled }),
            checkUpdate: () => httpRequest('POST', '/core/app/check-update'),
            fetchRemoteConfig: (url) => httpRequest('POST', '/core/app/fetch-remote-config', { url }),
            relaunch: () => httpRequest('POST', '/core/app/relaunch'),
        },
        setThemeSource: (mode) => httpRequest('POST', '/core/theme', { mode }),
        dialog: {
            showOpenDialog: (options) => httpRequest('POST', '/core/dialog/open', options),
        },
        clipboard: {
            copyImage: (dataUrl) => httpRequest('POST', '/core/clipboard/copy-image', { dataUrl }),
        },
    };
    
    // window.electron.* (Electron-specific IPC - stub implementations)
    window.electron = {
        ipcRenderer: {
            send: (channel, ...args) => console.log(`[Electron stub] ipcRenderer.send('${channel}')`, args),
            invoke: (channel, ...args) => {
                console.log(`[Electron stub] ipcRenderer.invoke('${channel}')`, args);
                return Promise.reject(new Error(`Electron IPC not available: ${channel}`));
            },
            on: (channel, listener) => console.log(`[Electron stub] ipcRenderer.on('${channel}')`),
            sendSync: (channel, ...args) => {
                console.log(`[Electron stub] ipcRenderer.sendSync('${channel}')`, args);
                return null;
            },
        },
        webUtils: {
            getPathForFile: (file) => {
                console.log('[Electron stub] webUtils.getPathForFile', file);
                return Promise.reject(new Error('webUtils.getPathForFile not available'));
            },
        },
    };
    
    // Backward compatibility helper.
    window.setThemeSource = (mode) => window.api.setThemeSource(mode);
    
    console.log('Xenobot IPC shim initialization complete');
})();
