import dayjs from 'dayjs'

export type SQLExportFormat = 'csv' | 'json'

export interface SQLExportData {
  columns: string[]
  rows: any[][]
}

/**
 * English note.
 */
export function formatAsCSV(data: SQLExportData): string {
  const header = data.columns.join(',')
  const rows = data.rows.map((row) =>
    row.map((cell) => (cell === null ? '' : `"${String(cell).replace(/"/g, '""')}"`)).join(',')
  )
  return [header, ...rows].join('\n')
}

/**
 * English note.
 */
export function formatAsJSON(data: SQLExportData): string {
  const jsonData = data.rows.map((row) => {
    const obj: Record<string, unknown> = {}
    data.columns.forEach((col, idx) => {
      obj[col] = row[idx]
    })
    return obj
  })
  return JSON.stringify(jsonData, null, 2)
}

/**
 * English note.
 */
export async function exportSQLResult(
  data: SQLExportData,
  format: SQLExportFormat
): Promise<{ success: boolean; filePath?: string; error?: string }> {
  if (data.rows.length === 0) {
    return { success: false, error: 'No data to export' }
  }

  const timestamp = dayjs().format('YYYYMMDD_HHmmss')
  const filename = `sql_result_${timestamp}.${format}`

  let content: string
  let mimeType: string

  if (format === 'json') {
    content = formatAsJSON(data)
    mimeType = 'application/json'
  } else {
    content = formatAsCSV(data)
    mimeType = 'text/csv'
  }

  // English engineering note.
  const dataUrl = `data:${mimeType};charset=utf-8,${encodeURIComponent(content)}`
  const result = await window.cacheApi.saveToDownloads(filename, dataUrl)

  return result
}
