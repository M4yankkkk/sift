export interface TaskItem {
  id: string;
  title: string;
  completed: boolean;
  priority: "low" | "medium" | "high";
}

export function filterActiveTasks(tasks: TaskItem[]): TaskItem[] {
  return tasks.filter((t) => !t.completed);
}
