export type NotificationType = "success" | "error" | "info";

export interface Notification {
  id: number;
  type: NotificationType;
  message: string;
}

export const notifications = $state<Notification[]>([]);

let nextNotificationId = 1;

export function notify(type: NotificationType, message: string): void {
  const id = nextNotificationId++;
  notifications.push({ id, type, message });
  window.setTimeout(() => dismissNotification(id), 6000);
}

export function dismissNotification(id: number): void {
  const index = notifications.findIndex(
    (notification) => notification.id === id,
  );
  if (index !== -1) {
    notifications.splice(index, 1);
  }
}
