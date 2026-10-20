// Lucide is ISC licensed. These original-size SVGs are rasterized by the native renderer.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  Terminal,
  Keyboard,
  Sparkles,
  Bot,
  Play,
  Sun,
  Circle,
} from "lucide-react";
const icons = Object.fromEntries(
  Object.entries({
    terminal: Terminal,
    keyboard: Keyboard,
    sparkles: Sparkles,
    bot: Bot,
    play: Play,
    sun: Sun,
    circle: Circle,
  }).map(([name, Icon]) => [
    name,
    renderToStaticMarkup(<Icon size={28} color="#e8eaef" />),
  ]),
);
await Bun.write(
  "src-tauri/assets/icons.json",
  JSON.stringify(icons, null, 2) + "\n",
);
