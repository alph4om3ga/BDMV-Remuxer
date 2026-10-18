import type { FileCategory } from '../types';
import { CATEGORIES } from '../types';

/**
 * Format seconds to HH:MM:SS or MM:SS
 */
export function formatDuration(seconds: number): string {
  if (!seconds || isNaN(seconds)) return '0:00';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) {
    return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }
  return `${m}:${String(s).padStart(2, '0')}`;
}

/**
 * Format bytes to human-readable size
 */
export function formatSize(bytes: number): string {
  if (!bytes || isNaN(bytes)) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let size = bytes;
  let unitIdx = 0;
  while (size >= 1024 && unitIdx < units.length - 1) {
    size /= 1024;
    unitIdx++;
  }
  return `${size.toFixed(unitIdx === 0 ? 0 : 1)} ${units[unitIdx]}`;
}

/**
 * Get the accent color for a file category
 */
export function getCategoryColor(category: string): string {
  return CATEGORIES[category as FileCategory]?.color ?? '#94a3b8';
}

/**
 * Get the episode/OP/ED/special number for a file
 */
export function getFileNumber(file: { category: FileCategory; episode_number?: number; op_number?: number; ed_number?: number; special_number?: number }): number | undefined {
  switch (file.category) {
    case 'EPISODE': return file.episode_number;
    case 'OP':      return file.op_number;
    case 'ED':      return file.ed_number;
    case 'SPECIAL': return file.special_number;
    default:        return undefined;
  }
}

/**
 * Format a file label like "E01", "OP1", "SP01"
 */
export function formatFileLabel(category: FileCategory, num?: number): string {
  if (num == null) return '';
  switch (category) {
    case 'EPISODE': return `E${String(num).padStart(2, '0')}`;
    case 'OP':      return `OP${num}`;
    case 'ED':      return `ED${num}`;
    case 'SPECIAL': return `SP${String(num).padStart(2, '0')}`;
    default:        return String(num);
  }
}
