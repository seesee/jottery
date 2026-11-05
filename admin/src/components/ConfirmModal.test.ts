import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import ConfirmModal from './ConfirmModal.svelte';

const base = {
  title: 'Remove thing',
  message: 'Really remove it?',
  confirmText: 'Remove',
  cancelText: 'Keep',
};

function show(props: Record<string, unknown> = {}) {
  const onConfirm = vi.fn();
  const onCancel = vi.fn();
  render(ConfirmModal, { props: { ...base, onConfirm, onCancel, ...props } });
  return { onConfirm, onCancel };
}

describe('ConfirmModal', () => {
  afterEach(cleanup);

  it('is announced as a modal dialog named by its title', () => {
    show();
    const dialog = screen.getByRole('dialog');
    expect(dialog.getAttribute('aria-modal')).toBe('true');
    const title = document.getElementById(dialog.getAttribute('aria-labelledby')!);
    expect(title?.textContent).toBe(base.title);
  });

  it('moves focus into the dialog and gives it back when it closes', async () => {
    const trigger = document.createElement('button');
    document.body.append(trigger);
    trigger.focus();

    show();
    expect(screen.getByRole('dialog').contains(document.activeElement)).toBe(true);

    cleanup();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });

  it('starts on Cancel when the action is dangerous, so Enter can\'t confirm it', () => {
    show({ danger: true });
    expect(document.activeElement).toBe(screen.getByRole('button', { name: base.cancelText }));
  });

  it('starts on the confirm button when the action is safe', () => {
    show();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: base.confirmText }));
  });

  it('cancels on Escape', async () => {
    const { onCancel, onConfirm } = show();
    await fireEvent.keyDown(document.activeElement!, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledOnce();
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('keeps Tab inside the dialog, in both directions', async () => {
    show();
    const cancel = screen.getByRole('button', { name: base.cancelText });
    const confirm = screen.getByRole('button', { name: base.confirmText });

    confirm.focus();
    await fireEvent.keyDown(confirm, { key: 'Tab' });
    expect(document.activeElement).toBe(cancel);

    await fireEvent.keyDown(cancel, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(confirm);
  });

  it('brings Tab back if focus has escaped the dialog', async () => {
    const outside = document.createElement('button');
    document.body.append(outside);
    show();
    outside.focus();

    await fireEvent.keyDown(outside, { key: 'Tab' });

    expect(screen.getByRole('dialog').contains(document.activeElement)).toBe(true);
    outside.remove();
  });

  it('only the top dialog answers Escape when two are open', async () => {
    const lower = show({ title: 'Lower' });
    const upper = show({ title: 'Upper' });

    await fireEvent.keyDown(document.activeElement!, { key: 'Escape' });

    expect(upper.onCancel).toHaveBeenCalledOnce();
    expect(lower.onCancel).not.toHaveBeenCalled();
  });

  it('shows the extra detail line when there is one', () => {
    show({ details: 'Notes: 12' });
    expect(screen.getByText('Notes: 12')).toBeTruthy();
  });

  describe('typing to confirm', () => {
    const typeToConfirm = { value: 'user@example.com', label: 'Type the address' };

    function confirmButton() {
      return screen.getByRole('button', { name: base.confirmText }) as HTMLButtonElement;
    }

    it('keeps the action disabled until the value has been typed', async () => {
      const { onConfirm } = show({ danger: true, typeToConfirm });
      expect(confirmButton().disabled).toBe(true);

      const field = screen.getByLabelText(typeToConfirm.label);
      await fireEvent.input(field, { target: { value: 'user@example.co' } });
      expect(confirmButton().disabled).toBe(true);

      await fireEvent.input(field, { target: { value: typeToConfirm.value } });
      expect(confirmButton().disabled).toBe(false);

      await fireEvent.click(confirmButton());
      expect(onConfirm).toHaveBeenCalledOnce();
    });

    it('doesn\'t confirm through the form until the value matches', async () => {
      const { onConfirm } = show({ danger: true, typeToConfirm });
      const field = screen.getByLabelText(typeToConfirm.label);

      await fireEvent.submit(field.closest('form')!);
      expect(onConfirm).not.toHaveBeenCalled();
    });

    it('starts on the field rather than Cancel', () => {
      show({ danger: true, typeToConfirm });
      expect(document.activeElement).toBe(screen.getByLabelText(typeToConfirm.label));
    });
  });

  describe('asking for a password', () => {
    const passwordLabel = 'Your password';

    it('passes what was typed to onConfirm, and needs something typed', async () => {
      const { onConfirm } = show({ danger: true, passwordLabel });
      const confirm = screen.getByRole('button', { name: base.confirmText }) as HTMLButtonElement;
      expect(confirm.disabled).toBe(true);

      await fireEvent.input(screen.getByLabelText(passwordLabel), { target: { value: 'hunter2hunter2' } });
      await fireEvent.click(confirm);

      expect(onConfirm).toHaveBeenCalledWith('hunter2hunter2');
    });

    it('uses a password field the browser fills from the signed-in account', () => {
      show({ danger: true, passwordLabel });
      const field = screen.getByLabelText(passwordLabel) as HTMLInputElement;
      expect(field.type).toBe('password');
      expect(field.autocomplete).toBe('current-password');
    });

    it('announces an error and stays open', () => {
      show({ danger: true, passwordLabel, error: 'Wrong password' });
      expect(screen.getByRole('alert').textContent).toContain('Wrong password');
    });

    it('can\'t be dismissed or resubmitted while the action is running', async () => {
      const { onCancel, onConfirm } = show({ danger: true, passwordLabel, busy: true });
      const field = screen.getByLabelText(passwordLabel) as HTMLInputElement;

      await fireEvent.keyDown(document.activeElement!, { key: 'Escape' });
      await fireEvent.submit(field.closest('form')!);

      expect(onCancel).not.toHaveBeenCalled();
      expect(onConfirm).not.toHaveBeenCalled();
    });
  });
});
