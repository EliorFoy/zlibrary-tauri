<script setup lang="ts">
import { ref } from "vue";
import { tasks, removeTask, clearDoneTasks, fmtBytes } from "../stores/downloadStore";
import { invoke } from "@tauri-apps/api/core";

const emit = defineEmits<{ "start-search": [] }>();
const failedImages = ref<Set<string>>(new Set());

function imgFailed(id: string) {
  failedImages.value = new Set([...failedImages.value, id]);
}

function showImg(id: string): boolean {
  return !failedImages.value.has(id);
}

async function openFolder(path: string) {
  if (path) {
    try {
      await invoke("open_file_location", { path });
    } catch (e: any) {
      const message = typeof e === "string" ? e : e?.message || "无法打开文件位置";
      window.alert(message);
    }
  }
}
</script>

<template>
  <div class="downloads-page">
    <div class="page-header">
      <h2>⬇ 下载管理</h2>
      <div class="header-actions">
        <button
          v-if="tasks.length > 0"
          class="btn-clear"
          @click="clearDoneTasks()"
        >
          清除已完成
        </button>
        <button class="btn-back" @click="emit('start-search')">
          ← 继续搜索
        </button>
      </div>
    </div>

    <!-- Empty state -->
    <div v-if="tasks.length === 0" class="empty-state">
      <span class="empty-icon">📥</span>
      <p>暂无下载任务</p>
      <button class="btn-search" @click="emit('start-search')">
        去搜索书籍
      </button>
    </div>

    <!-- Task list -->
    <div v-else class="task-list">
      <div
        v-for="task in tasks"
        :key="task.id"
        class="task-card"
        :class="{ 'task-error': task.status === 'error' }"
      >
        <div class="task-info">
          <img
            v-if="task.book.image_url && showImg(task.id)"
            :src="task.book.image_url"
            :alt="task.book.title"
            class="task-cover"
            @error="imgFailed(task.id)"
          />
          <div v-else class="task-cover-placeholder">📖</div>
          <div class="task-meta">
            <div class="task-title">{{ task.book.title }}</div>
            <div class="task-author">{{ task.book.author || "未知作者" }}</div>
            <div class="task-tags">
              <span v-if="task.book.extension" class="tag">{{ task.book.extension }}</span>
              <span v-if="task.book.file_size" class="tag">{{ task.book.file_size }}</span>
            </div>
          </div>
        </div>

        <div class="task-progress">
          <!-- Queued -->
          <div v-if="task.status === 'queued'" class="status-queued">
            <span class="spinner-sm"></span>
            <span>等待中…</span>
          </div>

          <!-- Downloading -->
          <div v-else-if="task.status === 'downloading'" class="status-downloading">
            <div class="progress-bar-wrap">
              <div
                class="progress-bar"
                :style="{ width: task.progress + '%' }"
              ></div>
            </div>
            <div class="progress-info">
              <span class="progress-pct">{{ task.progress }}%</span>
              <span class="progress-bytes">
                {{ fmtBytes(task.downloadedBytes) }} / {{ fmtBytes(task.totalBytes) }}
              </span>
            </div>
          </div>

          <!-- Done -->
          <div v-else-if="task.status === 'done'" class="status-done">
            <div class="done-row">
              <span class="done-badge">✅ 已完成</span>
              <span class="progress-pct done-pct">100%</span>
            </div>
            <div v-if="task.savePath" class="save-path-row">
              <span class="path-label">保存路径：</span>
              <span class="path-value">{{ task.savePath }}</span>
              <button class="btn-folder" @click="openFolder(task.savePath)">
                📂
              </button>
            </div>
          </div>

          <!-- Error -->
          <div v-else-if="task.status === 'error'" class="status-error">
            <div class="error-badge">❌ 下载失败</div>
            <div class="error-msg">{{ task.errorMsg }}</div>
          </div>
        </div>

        <div class="task-actions">
          <template v-if="task.status === 'done' || task.status === 'error'">
            <button class="btn-remove" @click="removeTask(task.id)">
              ✕
            </button>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.downloads-page {
  width: 100%;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 24px;
  flex-wrap: wrap;
  gap: 12px;
}

.page-header h2 {
  font-size: 1.3rem;
  color: var(--accent);
}

.header-actions {
  display: flex;
  gap: 8px;
}

.btn-clear {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border);
  padding: 8px 16px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 0.85rem;
  transition: all 0.2s;
}

.btn-clear:hover {
  color: var(--accent);
  border-color: var(--accent);
}

.btn-back {
  background: var(--accent);
  color: white;
  border: none;
  padding: 8px 18px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 0.85rem;
  font-weight: 600;
  transition: background 0.2s;
}

.btn-back:hover {
  background: var(--accent-hover);
}

.empty-state {
  text-align: center;
  padding: 80px 24px;
  color: var(--text-secondary);
}

.empty-icon {
  font-size: 4rem;
  display: block;
  margin-bottom: 16px;
  opacity: 0.4;
}

.btn-search {
  margin-top: 16px;
  padding: 12px 28px;
  background: linear-gradient(135deg, var(--accent-secondary), var(--accent));
  border: none;
  border-radius: 12px;
  color: white;
  font-size: 0.95rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-search:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-accent);
}

/* Task list */
.task-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.task-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 16px;
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  transition: border-color 0.2s;
}

.task-card:hover {
  border-color: var(--accent-secondary);
}

.task-error {
  border-color: var(--accent);
  opacity: 0.9;
}

.task-info {
  display: flex;
  gap: 14px;
  align-items: center;
}

.task-cover {
  width: 52px;
  height: 72px;
  border-radius: 8px;
  object-fit: cover;
  flex-shrink: 0;
}

.task-cover-placeholder {
  width: 52px;
  height: 72px;
  border-radius: 8px;
  background: linear-gradient(135deg, var(--accent-secondary), transparent);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.5rem;
  flex-shrink: 0;
}

.task-meta {
  flex: 1;
  min-width: 0;
}

.task-title {
  font-size: 0.95rem;
  font-weight: 600;
  display: -webkit-box;
  -webkit-line-clamp: 1;
  -webkit-box-orient: vertical;
  overflow: hidden;
  margin-bottom: 2px;
}

.task-author {
  font-size: 0.8rem;
  color: var(--accent);
  margin-bottom: 6px;
}

.task-tags {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.tag {
  padding: 2px 8px;
  background: var(--badge-bg);
  border: 1px solid var(--badge-border);
  border-radius: 6px;
  font-size: 0.7rem;
  color: var(--badge-text);
}

/* Progress */
.task-progress {
  padding-left: 66px;
}

.status-queued {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-secondary);
  font-size: 0.85rem;
}

.spinner-sm {
  width: 14px;
  height: 14px;
  border: 2px solid var(--badge-border);
  border-top-color: var(--accent-secondary);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
  display: inline-block;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.progress-bar-wrap {
  height: 8px;
  background: var(--bg-primary);
  border-radius: 8px;
  overflow: hidden;
  margin-bottom: 6px;
}

.progress-bar {
  height: 100%;
  background: linear-gradient(90deg, var(--accent-secondary), var(--accent));
  border-radius: 8px;
  transition: width 0.3s ease;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  font-size: 0.8rem;
}

.progress-pct {
  color: var(--accent-secondary);
  font-weight: 700;
}

.progress-bytes {
  color: var(--text-secondary);
}

/* Done */
.status-done .done-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 6px;
}

.done-badge {
  color: #4ade80;
  font-size: 0.85rem;
  font-weight: 600;
}

.done-pct {
  font-size: 0.8rem;
}

.save-path-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.8rem;
  color: var(--text-secondary);
  word-break: break-all;
}

.path-label {
  flex-shrink: 0;
}

.path-value {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.btn-folder {
  background: none;
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 4px 8px;
  cursor: pointer;
  font-size: 0.9rem;
  flex-shrink: 0;
  transition: all 0.2s;
}

.btn-folder:hover {
  border-color: var(--accent-secondary);
  background: var(--badge-bg);
}

/* Error */
.status-error .error-badge {
  color: var(--accent);
  font-size: 0.85rem;
  font-weight: 600;
  margin-bottom: 4px;
}

.status-error .error-msg {
  font-size: 0.8rem;
  color: var(--text-secondary);
}

/* Actions */
.task-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.btn-remove {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border);
  width: 32px;
  height: 32px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 0.85rem;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.btn-remove:hover {
  color: var(--accent);
  border-color: var(--accent);
  background: rgba(233, 69, 96, 0.1);
}

/* Responsive */
@media (max-width: 768px) {
  .page-header {
    flex-direction: column;
    align-items: flex-start;
  }

  .task-progress {
    padding-left: 0;
  }

  .task-card {
    padding: 12px 14px;
  }
}
</style>
