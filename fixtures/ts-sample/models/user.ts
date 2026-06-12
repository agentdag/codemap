export interface Named {
  label(): string;
}

export type UserId = string | number;

export class User {
  id: UserId = "";

  greet(): string {
    return "hi";
  }

  rename(next: string): void {
    this.id = next;
  }
}

export function defaultUser(): User {
  return build();
}

function build(): User {
  return new User();
}
