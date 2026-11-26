// Pages and the Google authentication proxy share the cloud builder. Locally
// served pages use their own server, including during offline development.
import { API_BASE } from "./config.js";

export const publicBuilder = location.origin === "https://yesint.github.io";
export const builderApi = publicBuilder
  ? "https://molar-builder-api-439771002960.europe-west1.run.app/api"
  : API_BASE.replace(/\/$/, "") || "/api";

export function structureUrl(url) {
  return url.startsWith("/api/") ? builderApi + url.slice(4) : url;
}
