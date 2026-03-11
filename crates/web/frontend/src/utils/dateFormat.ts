import dayjs from "dayjs";

/**
 * English note.
 * English note.
 */

/**
 * English note.
 * English note.
 */
export function formatDate(ts: number): string {
  const date = new Date(ts * 1000);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/**
 * English note.
 * English note.
 */
export function formatDateTime(ts: number): string {
  const date = new Date(ts * 1000);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hour = String(date.getHours()).padStart(2, "0");
  const minute = String(date.getMinutes()).padStart(2, "0");
  return `${year}-${month}-${day} ${hour}:${minute}`;
}

/**
 * English note.
 * English note.
 */
export function formatFullDateTime(ts: number): string {
  return dayjs.unix(ts).format("YYYY-MM-DD HH:mm:ss");
}

/**
 * English note.
 * English note.
 * English note.
 */
export function formatPeriod(startTs: number, endTs: number | null): string {
  const start = formatDate(startTs);
  if (endTs === null) {
    return `${start} ~ 至今`;
  }
  const end = formatDate(endTs);
  if (start === end) {
    return start;
  }
  return `${start} ~ ${end}`;
}

/**
 * English note.
 * English note.
 */
export function formatDaysSince(days: number): string {
  if (days === 0) return "今天";
  if (days === 1) return "昨天";
  if (days < 30) return `${days} 天前`;
  if (days < 365) return `${Math.floor(days / 30)} 个月前`;
  return `${Math.floor(days / 365)} 年前`;
}

/**
 * English note.
 * English note.
 * English note.
 */
export function formatWithDayjs(ts: number, format: string): string {
  return dayjs.unix(ts).format(format);
}

/**
 * English note.
 * English note.
 * English note.
 * English note.
 * English note.
 */
export function formatDateRange(
  startTs: number,
  endTs: number,
  format: string = "YYYY/MM/DD",
  separator: string = " - ",
): string {
  const start = dayjs.unix(startTs).format(format);
  const end = dayjs.unix(endTs).format(format);
  if (start === end) {
    return start;
  }
  return `${start}${separator}${end}`;
}
