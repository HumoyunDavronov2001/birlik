// The panel's short notices (DS-05): a device joined, a device was removed, something failed. They stack in a corner,
// are read out politely (an error at once), and go away by themselves unless they ask for an action.

export type ToastKind = 'good' | 'info' | 'warning' | 'critical';

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  body?: string;
}

class Toasts {
  list = $state<Toast[]>([]);
  private next = 1;

  /** Shows a notice for `timeout` milliseconds (0: until closed). Returns its id. */
  show(toast: Omit<Toast, 'id'>, timeout = 6000): number {
    const id = this.next++;
    this.list = [...this.list, { ...toast, id }].slice(-4);
    if (timeout > 0) setTimeout(() => this.dismiss(id), timeout);
    return id;
  }

  dismiss(id: number): void {
    this.list = this.list.filter((toast) => toast.id !== id);
  }
}

export const toasts = new Toasts();
