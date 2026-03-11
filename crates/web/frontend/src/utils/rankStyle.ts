/**
 * English note.
 * English note.
 */

// English engineering note.
export function getRankBadgeClass(index: number): string {
  if (index === 0)
    return "bg-linear-to-r from-amber-400 to-orange-500 text-white";
  if (index === 1) return "bg-linear-to-r from-gray-300 to-gray-400 text-white";
  if (index === 2)
    return "bg-linear-to-r from-amber-600 to-amber-700 text-white";
  return "bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400";
}

// English engineering note.
export function getRankBarColor(index: number): string {
  const colors = [
    "from-amber-400 to-orange-500",
    "from-gray-300 to-gray-400",
    "from-amber-600 to-amber-700",
    "from-pink-400 to-pink-600",
    "from-pink-300 to-rose-500",
    "from-cyan-400 to-blue-500",
    "from-green-400 to-emerald-500",
    "from-rose-400 to-pink-500",
  ];
  return colors[index % colors.length];
}
