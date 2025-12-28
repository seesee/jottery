// Toast notification store
interface Toast {
  id: number;
  message: string;
  type: 'success' | 'error' | 'info';
}

class ToastStore {
  private toasts = $state<Toast[]>([]);
  private nextId = 0;

  get items() {
    return this.toasts;
  }

  show(message: string, type: Toast['type'] = 'info') {
    const id = this.nextId++;
    this.toasts = [...this.toasts, { id, message, type }];

    // Auto-remove after 5 seconds
    setTimeout(() => {
      this.remove(id);
    }, 5000);
  }

  success(message: string) {
    this.show(message, 'success');
  }

  error(message: string) {
    this.show(message, 'error');
  }

  info(message: string) {
    this.show(message, 'info');
  }

  remove(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

export const toast = new ToastStore();
export type { Toast };
