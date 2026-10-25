'use strict';

// Pure adaptive policy: isolated so its decisions can be unit-tested without Win32.
const MIN_CAP_MB = 12;
const MAX_CAP_MB = 64;
const INCREASE_STEP_MB = 8;
const DECREASE_STEP_MB = 4;
const HIGH_CPU_PERCENT = 10;
const LOW_CPU_PERCENT = 3;
const HIGH_CPU_DURATION_MS = 6000;
const LOW_CPU_DURATION_MS = 10000;
const ADJUSTMENT_COOLDOWN_MS = 8000;
const LOW_WORKING_SET_MARGIN_MB = 6;

const BASE_CAP_MB = Object.freeze({
  Main: 36,
  GPU: 28,
  Renderer: 28,
  Utility: 24,
  Crashpad: 16,
  Helper: 24
});

// Keep foreground-critical code resident enough to limit restore latency.
const ROLE_FLOOR_MB = Object.freeze({
  Main: 24,
  GPU: 16,
  Renderer: 12,
  Utility: 12,
  Crashpad: 12,
  Helper: 16
});

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function baseCapMB(role) {
  return BASE_CAP_MB[role] || BASE_CAP_MB.Helper;
}

function estimateCpuPercent(previousCpuMs, currentCpuMs, elapsedMs) {
  const previous = Number(previousCpuMs);
  const current = Number(currentCpuMs);
  const elapsed = Number(elapsedMs);
  if (!Number.isFinite(previous) || !Number.isFinite(current) || !Number.isFinite(elapsed) || elapsed <= 0 || current < previous) return null;
  // Percentage of one logical processor, capped at 100 for the per-process UI metric.
  return clamp(((current - previous) / elapsed) * 100, 0, 100);
}

function maximumCapMB(role) {
  if (role === 'Main') return baseCapMB(role);
  return Math.min(MAX_CAP_MB, baseCapMB(role) + 24);
}

function minimumCapMB(role) {
  return Math.max(MIN_CAP_MB, ROLE_FLOOR_MB[role] || ROLE_FLOOR_MB.Helper);
}

function decideAdjustment(sample) {
  const role = String(sample.role || 'Helper');
  const currentCapMB = Number(sample.currentCapMB);
  const workingSetMB = Number(sample.workingSetMB);
  const cpuPercent = Number(sample.cpuPercent);
  const highCpuMs = Number(sample.highCpuMs) || 0;
  const lowCpuMs = Number(sample.lowCpuMs) || 0;
  const lastAdjustmentAt = Number(sample.lastAdjustmentAt) || 0;
  const now = Number(sample.now) || Date.now();

  if (!Number.isFinite(currentCapMB) || !Number.isFinite(workingSetMB) || !Number.isFinite(cpuPercent)) return null;
  if (now - lastAdjustmentAt < ADJUSTMENT_COOLDOWN_MS) return null;

  if (cpuPercent >= HIGH_CPU_PERCENT && highCpuMs >= HIGH_CPU_DURATION_MS &&
      workingSetMB >= currentCapMB) {
    const next = Math.min(maximumCapMB(role), currentCapMB + INCREASE_STEP_MB);
    return next > currentCapMB
      ? { nextCapMB: next, direction: 'increase', reason: 'sustained-high-cpu' }
      : null;
  }

  if (cpuPercent <= LOW_CPU_PERCENT && lowCpuMs >= LOW_CPU_DURATION_MS &&
      workingSetMB <= currentCapMB - LOW_WORKING_SET_MARGIN_MB) {
    const next = Math.max(minimumCapMB(role), currentCapMB - DECREASE_STEP_MB);
    return next < currentCapMB
      ? { nextCapMB: next, direction: 'decrease', reason: 'low-cpu-and-memory-headroom' }
      : null;
  }

  return null;
}

module.exports = {
  MIN_CAP_MB,
  MAX_CAP_MB,
  BASE_CAP_MB,
  baseCapMB,
  estimateCpuPercent,
  maximumCapMB,
  minimumCapMB,
  decideAdjustment
};
