import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";

// Development screenshots (browser + mock backend only): disable animations with ?shot.
if (!("__TAURI_INTERNALS__" in window) && new URLSearchParams(location.search).has("shot")) {
  document.documentElement.classList.add("no-anim");
}

const app = mount(App, { target: document.getElementById("app")! });

export default app;
