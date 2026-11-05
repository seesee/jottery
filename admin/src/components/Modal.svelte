<script module lang="ts">
  // Dialogs can open on top of each other, and only the top one takes the keyboard.
  const openDialogs: object[] = [];

  const FOCUSABLE = [
    'a[href]',
    'button:not([disabled])',
    'input:not([disabled])',
    'select:not([disabled])',
    'textarea:not([disabled])',
    '[tabindex]:not([tabindex="-1"])',
  ].join(',');
</script>

<script lang="ts">
  import { onMount, type Snippet } from 'svelte';

  // The behaviour every dialog shares: it's announced as a dialog, takes focus when it
  // opens and gives it back when it closes, keeps Tab inside, and closes on Escape.
  // Put data-initial-focus on the element that should start with focus. Without one,
  // the first control gets it.

  interface Props {
    labelledBy: string;
    onClose: () => void;
    /** Stops Escape closing the dialog, while an action is running. */
    locked?: boolean;
    overlayClass?: string;
    panelClass?: string;
    children: Snippet;
  }

  let {
    labelledBy,
    onClose,
    locked = false,
    overlayClass = '',
    panelClass = '',
    children,
  }: Props = $props();

  const dialog = {};
  let panel: HTMLElement;

  function focusableElements(): HTMLElement[] {
    return Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE));
  }

  onMount(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    openDialogs.push(dialog);

    const target = panel.querySelector<HTMLElement>('[data-initial-focus]') ?? focusableElements()[0] ?? panel;
    target.focus();

    return () => {
      openDialogs.splice(openDialogs.indexOf(dialog), 1);
      previouslyFocused?.focus?.();
    };
  });

  function keepTabInside(event: KeyboardEvent) {
    const elements = focusableElements();
    if (elements.length === 0) {
      event.preventDefault();
      panel.focus();
      return;
    }

    const first = elements[0];
    const last = elements[elements.length - 1];
    const active = document.activeElement;
    const leaving = event.shiftKey ? active === first : active === last;

    if (leaving || !panel.contains(active)) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (openDialogs[openDialogs.length - 1] !== dialog) {
      return;
    }
    if (event.key === 'Tab') {
      keepTabInside(event);
    } else if (event.key === 'Escape' && !locked) {
      event.preventDefault();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 {overlayClass}">
  <div
    bind:this={panel}
    role="dialog"
    aria-modal="true"
    aria-labelledby={labelledBy}
    tabindex="-1"
    class="bg-white rounded-lg shadow-xl focus:outline-none {panelClass}"
  >
    {@render children()}
  </div>
</div>
