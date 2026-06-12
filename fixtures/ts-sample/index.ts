import { User, UserId, defaultUser } from "./models/user";

export function makeUser(id: UserId): User {
  const u = defaultUser();
  u.rename(String(id));
  console.log(u.greet());
  return u;
}

export interface AppConfig {
  debug: boolean;
}
