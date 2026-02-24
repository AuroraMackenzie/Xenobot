import type { RepeatAnalysis } from '@/types/analysis'

interface TimeFilter {
  startTs?: number
  endTs?: number
}

interface SQLResult {
  columns: string[]
  rows: unknown[][]
}

export type XenoRepeatAnalysis = RepeatAnalysis

function toNumber(value: unknown, fallback = 0): number {
  const n = Number(value)
  return Number.isFinite(n) ? n : fallback
}

function toStringValue(value: unknown, fallback = ''): string {
  return value === null || value === undefined ? fallback : String(value)
}

function getColumnIndex(result: SQLResult, column: string): number {
  return result.columns.findIndex((name) => name === column)
}

function emptyRepeatAnalysis(): RepeatAnalysis {
  return {
    originators: [],
    initiators: [],
    breakers: [],
    fastestRepeaters: [],
    originatorRates: [],
    initiatorRates: [],
    breakerRates: [],
    chainLengthDistribution: [],
    hotContents: [],
    avgChainLength: 0,
    totalRepeatChains: 0,
  }
}

export async function queryXenoRepeatAnalysis(
  sessionId: string,
  timeFilter?: TimeFilter
): Promise<XenoRepeatAnalysis> {
  const metaId = Number.parseInt(sessionId, 10)
  if (!Number.isFinite(metaId)) {
    return emptyRepeatAnalysis()
  }

  const clauses = [`meta_id = ${Math.trunc(metaId)}`, `content IS NOT NULL`, `TRIM(content) <> ''`]
  if (Number.isFinite(Number(timeFilter?.startTs))) {
    clauses.push(`ts >= ${Math.trunc(Number(timeFilter?.startTs))}`)
  }
  if (Number.isFinite(Number(timeFilter?.endTs))) {
    clauses.push(`ts <= ${Math.trunc(Number(timeFilter?.endTs))}`)
  }

  const sql = `
    SELECT
      TRIM(content) AS content,
      COUNT(*) AS repeat_count,
      MIN(id) AS first_message_id,
      MAX(ts) AS last_ts,
      MIN(COALESCE(sender_group_nickname, sender_account_name, 'Unknown')) AS originator_name
    FROM message
    WHERE ${clauses.join(' AND ')}
    GROUP BY TRIM(content)
    HAVING COUNT(*) >= 2
    ORDER BY repeat_count DESC, last_ts DESC
    LIMIT 50
  `

  try {
    const result = (await window.chatApi.executeSQL(sessionId, sql)) as SQLResult
    if (!result || !Array.isArray(result.rows) || result.rows.length === 0) {
      return emptyRepeatAnalysis()
    }

    const idxContent = getColumnIndex(result, 'content')
    const idxCount = getColumnIndex(result, 'repeat_count')
    const idxFirstMessageId = getColumnIndex(result, 'first_message_id')
    const idxLastTs = getColumnIndex(result, 'last_ts')
    const idxOriginator = getColumnIndex(result, 'originator_name')

    const hotContents = result.rows.map((row) => {
      const count = toNumber(row[idxCount], 0)
      return {
        content: toStringValue(row[idxContent], ''),
        count,
        maxChainLength: Math.max(2, Math.min(count, 12)),
        originatorName: toStringValue(row[idxOriginator], 'Unknown'),
        lastTs: toNumber(row[idxLastTs], 0),
        firstMessageId: toNumber(row[idxFirstMessageId], 0),
      }
    })

    const totalRepeatChains = hotContents.reduce((sum, item) => sum + item.count, 0)
    const avgChainLength = hotContents.length
      ? hotContents.reduce((sum, item) => sum + item.maxChainLength, 0) / hotContents.length
      : 0

    return {
      ...emptyRepeatAnalysis(),
      hotContents,
      totalRepeatChains,
      avgChainLength,
    }
  } catch (error) {
    console.error('queryXenoRepeatAnalysis failed:', error)
    return emptyRepeatAnalysis()
  }
}
