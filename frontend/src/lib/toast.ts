export interface Toast {
  id: number;
  type: 'error' | 'success' | 'info';
  message: string;
}
