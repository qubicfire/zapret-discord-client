import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const fill = document.getElementById("fill") as HTMLElement;
const status = document.getElementById("status") as HTMLElement;
const percentage = document.getElementById("percentage") as HTMLElement;

window.addEventListener("DOMContentLoaded", async () => {
  invoke("start_update");

  await listen("download-progress", (event: any) => {
    const { progress, total, status: statusText } = event.payload;
    const percent = Math.round((progress / total) * 100);

    console.log(progress / (1024 * 1024))
    console.log(total / (1024 * 1024))

    fill.style.width = `${percent}%`;
    percentage.textContent = `${percent}% (${(progress / (1024 * 1024)).toFixed(2)} / ${(total / (1024 * 1024)).toFixed(2)} MB)`;
    status.textContent = statusText;
  });

  await listen("update-finished", () => {
    status.textContent = "Обновление завершено!";
    percentage.textContent = "100%";
  });
});