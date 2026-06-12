import { User, UserId } from "./models/user";

export function makeUser(id: UserId): User {
  const u = new User();
  u.rename(String(id));
  return u;
}

export interface AppConfig {
  debug: boolean;
}
