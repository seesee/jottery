// Toast notification store
interface ToastAction {
  label: string;
  onClick: () => void;
}

interface Toast {
  id: number;
  message: string;
  type: 'success' | 'error' | 'info';
  action?: ToastAction;
}

class ToastStore {
  private toasts = $state<Toast[]>([]);
  private nextId = 0;

  get items() {
    return this.toasts;
  }

  show(message: string, type: Toast['type'] = 'info', action?: ToastAction) {
    const id = this.nextId++;
    this.toasts = [...this.toasts, { id, message, type, action }];

    // Auto-remove after 5 seconds (actionable toasts linger longer)
    setTimeout(() => {
      this.remove(id);
    }, action ? 10000 : 5000);
  }

  success(message: string, action?: ToastAction) {
    this.show(message, 'success', action);
  }

  error(message: string, action?: ToastAction) {
    this.show(message, 'error', action);
  }

  info(message: string, action?: ToastAction) {
    this.show(message, 'info', action);
  }

  remove(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

export const toast = new ToastStore();
export type { Toast, ToastAction };
