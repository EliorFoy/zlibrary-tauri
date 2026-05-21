<script setup lang="ts">
import { ref, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { BookInfo } from "../types";

const props = defineProps<{ book: BookInfo }>();
const emit = defineEmits<{ back: [] }>();

const downloading = ref(false);
const progress = ref(0);
const progressMsg = ref("");
const done = ref(false);
const error = ref("");
const savePath = ref("");

let unlisten: (() => void) | null = null;

async function startDownload() {
  downloading.value = true;
  error.value = "";
  progress.value = 0;

  try {
    const result = await invoke<string>("download_book", {
      book: props.book,
    });
    savePath.value = result;
    done.value = true;
    progress.value = 100;
    progressMsg.value = "下载完成";
  } catch (e: any) {
    error.value = typeof e === "string" ? e : e?.message || "下载失败";
    downloading.value = false;
  }
}

function openFolder() {
  if (savePath.value) {
    invoke("open_file_location", { path: savePath.value });
  }
}

onUnmounted(() => {
  unlisten?.();
});
</script>

<template>
  <div class="download-view">
    <button class="back-btn" @click="emit('back')">
      <span>←</span> 返回搜索
    </button>

    <div class="download-card">
      <div class="book-info">
        <img
          v-if="book.image_url"
          :src="book.image_url"
          :alt="book.title"
          class="book-cover"
        />
        <div v-else class="book-cover-placeholder">📖</div>

        <div class="book-details">
          <h2 class="book-title">{{ book.title }}</h2>
          <p class="book-author">{{ book.author || "未知作者" }}</p>
          <div class="book-meta">
            <span v-if="book.extension" class="meta-tag">{{ book.extension }}</span>
            <span v-if="book.file_size" class="meta-tag">{{ book.file_size }}</span>
            <span v-if="book.language" class="meta-tag">{{ book.language }}</span>
            <span v-if="book.year" class="meta-tag">{{ book.year }}</span>
          </div>
        </div>
      </div>

      <div class="download-section">
        <div v-if="!downloading && !done" class="download-actions">
          <button class="download-btn" @click="startDownload">
            <span>⬇</span> 开始下载
          </button>
        </div>

        <div v-else-if="downloading && !done" class="progress-section">
          <div class="progress-bar-wrap">
            <div class="progress-bar" :style="{ width: progress + '%' }"></div>
          </div>
          <p class="progress-text">{{ progressMsg || "准备下载…" }}</p>
        </div>

        <div v-else-if="done" class="done-section">
          <div class="done-icon">✅</div>
          <h3>下载完成</h3>
          <p class="save-path">{{ savePath }}</p>
          <div class="done-actions">
            <button class="folder-btn" @click="openFolder">📂 打开文件夹</button>
            <button class="back-btn-secondary" @click="emit('back')">继续搜索</button>
          </div>
        </div>

        <div v-if="error" class="download-error">{{ error }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.download-view {
  animation: fadeIn 0.3s ease;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(12px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.back-btn {
  background: none;
  border: 1px solid var(--border);
  color: var(--text-secondary);
  padding: 10px 20px;
  border-radius: 12px;
  cursor: pointer;
  font-size: 0.9rem;
  transition: all 0.2s;
  margin-bottom: 24px;
}

.back-btn:hover {
  border-color: #7c4dff;
  color: var(--text-primary);
  background: #7c4dff10;
}

.download-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 20px;
  padding: 32px;
}

.book-info {
  display: flex;
  gap: 24px;
  margin-bottom: 32px;
}

.book-cover {
  width: 120px;
  height: 170px;
  border-radius: 12px;
  object-fit: cover;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
}

.book-cover-placeholder {
  width: 120px;
  height: 170px;
  border-radius: 12px;
  background: linear-gradient(135deg, #7c4dff30, #e9456030);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 3rem;
}

.book-details {
  flex: 1;
}

.book-title {
  font-size: 1.3rem;
  font-weight: 700;
  margin-bottom: 8px;
  line-height: 1.4;
}

.book-author {
  color: var(--accent);
  font-size: 0.95rem;
  margin-bottom: 16px;
}

.book-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.meta-tag {
  padding: 4px 12px;
  background: #7c4dff15;
  border: 1px solid #7c4dff30;
  border-radius: 8px;
  font-size: 0.8rem;
  color: #7c4dff;
}

.download-section {
  border-top: 1px solid var(--border);
  padding-top: 24px;
}

.download-actions {
  text-align: center;
}

.download-btn {
  padding: 16px 48px;
  background: linear-gradient(135deg, #7c4dff, #e94560);
  border: none;
  border-radius: 16px;
  color: white;
  font-size: 1.1rem;
  font-weight: 600;
  cursor: pointer;
  transition: transform 0.2s, box-shadow 0.2s;
}

.download-btn:hover {
  transform: translateY(-2px);
  box-shadow: 0 8px 24px rgba(124, 77, 255, 0.35);
}

.progress-section {
  text-align: center;
}

.progress-bar-wrap {
  height: 10px;
  background: var(--bg-primary);
  border-radius: 10px;
  overflow: hidden;
  margin-bottom: 12px;
}

.progress-bar {
  height: 100%;
  background: linear-gradient(90deg, #7c4dff, #e94560);
  border-radius: 10px;
  transition: width 0.3s ease;
}

.progress-text {
  color: var(--text-secondary);
  font-size: 0.9rem;
}

.done-section {
  text-align: center;
}

.done-icon {
  font-size: 3rem;
  margin-bottom: 12px;
}

.done-section h3 {
  font-size: 1.2rem;
  margin-bottom: 8px;
  color: #4ade80;
}

.save-path {
  color: var(--text-secondary);
  font-size: 0.85rem;
  margin-bottom: 20px;
  word-break: break-all;
  padding: 8px 16px;
  background: var(--bg-primary);
  border-radius: 8px;
  display: inline-block;
}

.done-actions {
  display: flex;
  gap: 12px;
  justify-content: center;
}

.folder-btn {
  padding: 12px 28px;
  background: linear-gradient(135deg, #7c4dff, #6366f1);
  border: none;
  border-radius: 12px;
  color: white;
  font-size: 0.95rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.folder-btn:hover {
  transform: translateY(-2px);
  box-shadow: 0 6px 20px rgba(124, 77, 255, 0.3);
}

.back-btn-secondary {
  padding: 12px 28px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  color: var(--text-primary);
  font-size: 0.95rem;
  cursor: pointer;
  transition: all 0.2s;
}

.back-btn-secondary:hover {
  border-color: #7c4dff;
}

.download-error {
  margin-top: 16px;
  padding: 12px 20px;
  background: rgba(233, 69, 96, 0.1);
  border: 1px solid rgba(233, 69, 96, 0.3);
  color: var(--accent);
  border-radius: 12px;
  text-align: center;
}
</style>