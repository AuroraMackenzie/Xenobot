<template>
  <section class="xeno-batch-safe-panel rounded-3xl p-8">
    <div class="xeno-batch-safe-badge">SAFE MODE</div>
    <h3 class="xeno-batch-safe-title">Destructive batch actions removed</h3>
    <p class="xeno-batch-safe-copy">
      Session deletion and destructive batch operations are disabled in the
      frontend. Use the import, monitoring, analysis, and export workflows
      instead.
    </p>
  </section>
</template>

<style scoped>
.xeno-batch-safe-panel {
  border: 1px solid var(--xeno-border-soft);
  background:
    radial-gradient(
      circle at top right,
      rgba(59, 130, 246, 0.08),
      transparent 24%
    ),
    var(--xeno-stage-shell-bg);
  box-shadow:
    inset 0 1px 0 var(--xeno-surface-hairline),
    0 18px 38px rgba(2, 6, 23, 0.14);
  backdrop-filter: none;
}

.xeno-batch-safe-badge {
  display: inline-flex;
  align-items: center;
  border: 1px solid rgba(56, 189, 248, 0.28);
  border-radius: 999px;
  padding: 0.2rem 0.65rem;
  color: rgb(56, 189, 248);
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.08em;
}

.xeno-batch-safe-title {
  margin-top: 1rem;
  color: var(--xeno-text-main);
  font-size: 1.25rem;
  font-weight: 700;
}

.xeno-batch-safe-copy {
  margin-top: 0.65rem;
  max-width: 42rem;
  color: var(--xeno-text-secondary);
  line-height: 1.7;
}
</style>
