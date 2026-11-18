<script lang="ts" generics="T extends string">
  interface Props {
    legend: string;
    name: string;
    options: { value: T; label: string }[];
    value: T;
    onchange?: (value: T) => void;
  }
  let { legend, name, options, value = $bindable(), onchange }: Props = $props();
</script>

<!-- Real radios: arrow keys move the selection, Tab enters and leaves the group. -->
<fieldset>
  <legend class="vh">{legend}</legend>
  <div class="track">
    {#each options as o (o.value)}
      <label>
        <input
          class="vh"
          type="radio"
          {name}
          value={o.value}
          checked={value === o.value}
          onchange={() => {
            value = o.value;
            onchange?.(o.value);
          }}
        />
        <span>{o.label}</span>
      </label>
    {/each}
  </div>
</fieldset>

<style>
  .track {
    display: inline-flex;
    height: var(--control-h);
    border: var(--hairline) solid var(--border);
    border-radius: var(--radius-control);
    overflow: hidden;
  }

  label {
    position: relative;
    display: flex;
    cursor: pointer;
  }

  label + label {
    border-left: var(--hairline) solid var(--line);
  }

  span {
    display: flex;
    align-items: center;
    padding: 0 var(--space-3);
    color: var(--text-muted);
    font-weight: var(--weight-strong);
    transition:
      background-color var(--motion-quick) var(--ease),
      color var(--motion-quick) var(--ease);
  }

  label:hover span {
    background: var(--bg-quiet);
  }

  input:checked + span {
    background: var(--bg-pressed);
    color: var(--text);
  }

  input:focus-visible + span {
    outline: var(--rule) solid var(--focus);
    outline-offset: calc(-1 * var(--rule));
  }
</style>
