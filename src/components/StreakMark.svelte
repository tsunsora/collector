<script lang="ts"></script>

<!-- Streak-pending marker. One of the two places the accent is allowed. -->
<span class="streak"><span class="mark" aria-hidden="true"></span>Streak due</span>

<style>
  .streak {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--accent);
    font-size: var(--text-caption);
    line-height: var(--lh-caption);
    font-weight: var(--weight-strong);
    white-space: nowrap;
  }

  .mark {
    width: var(--space-2);
    height: var(--space-2);
    background: var(--accent);
  }
</style>
