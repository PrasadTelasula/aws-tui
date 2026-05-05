<script lang="ts">
  interface Props {
    tone: 'ok' | 'warn' | 'error' | 'info' | 'muted';
    pulse?: boolean;
    size?: number;
  }

  let { tone, pulse = false, size = 8 }: Props = $props();
</script>

<span
  class={`tui-dot tui-dot-${tone}` + (pulse ? ' tui-dot-pulse' : '')}
  style="width: {size}px; height: {size}px;"
></span>
