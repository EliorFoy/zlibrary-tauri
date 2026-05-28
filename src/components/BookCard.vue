<script setup lang="ts">
import { ref } from "vue";
import type { BookInfo } from "../types";

defineProps<{ book: BookInfo; disabled?: boolean }>();
const emit = defineEmits<{ download: [book: BookInfo] }>();
const imgFailed = ref(false);

function ratingColor(rating: string): string {
  const r = parseFloat(rating);
  if (r >= 4.5) return "#4ade80";
  if (r >= 3) return "#facc15";
  return "var(--text-secondary)";
}
</script>

<template>
  <div class="book-card">
    <div class="card-header">
      <img
        v-if="book.image_url && !imgFailed"
        :src="book.image_url"
        :alt="book.title"
        class="card-cover"
        @error="imgFailed = true"
      />
      <div v-else class="card-cover-placeholder">📕</div>
      <div class="card-meta">
        <span v-if="book.extension" class="badge">{{ book.extension }}</span>
        <span v-if="book.file_size" class="badge">{{ book.file_size }}</span>
        <span v-if="book.year" class="badge">{{ book.year }}</span>
      </div>
    </div>

    <div class="card-body">
      <h3 class="card-title" :title="book.title">{{ book.title }}</h3>
      <p class="card-author">{{ book.author || "未知作者" }}</p>
      <div class="card-footer">
        <span v-if="book.rating" class="rating" :style="{ color: ratingColor(book.rating) }">
          ★ {{ book.rating }}
        </span>
        <span v-if="book.language" class="lang">{{ book.language }}</span>
      </div>
    </div>

    <button class="card-btn" :disabled="disabled" @click="emit('download', book)">
      <span>{{ disabled ? "⏳" : "⬇" }}</span>
      {{ disabled ? "下载中…" : "下载" }}
    </button>
  </div>
</template>

<style scoped>
.book-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 16px;
  padding: 20px;
  display: flex;
  gap: 16px;
  align-items: flex-start;
  transition: all 0.25s ease;
  position: relative;
  overflow: hidden;
}

.book-card::before {
  content: "";
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(135deg, var(--badge-bg), transparent);
  opacity: 0;
  transition: opacity 0.25s;
}

.book-card:hover {
  transform: translateY(-2px);
  border-color: var(--accent-secondary);
  box-shadow: var(--shadow-accent);
}

.book-card:hover::before {
  opacity: 1;
}

.card-header {
  position: relative;
  flex-shrink: 0;
  z-index: 1;
}

.card-cover {
  width: 80px;
  height: 112px;
  border-radius: 10px;
  object-fit: cover;
  box-shadow: var(--shadow-card);
}

.card-cover-placeholder {
  width: 80px;
  height: 112px;
  border-radius: 10px;
  background: linear-gradient(135deg, var(--accent-secondary), transparent);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 2rem;
}

.card-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 6px;
}

.badge {
  padding: 2px 8px;
  background: var(--badge-bg);
  border: 1px solid var(--badge-border);
  border-radius: 6px;
  font-size: 0.7rem;
  color: var(--badge-text);
}

.card-body {
  flex: 1;
  min-width: 0;
  z-index: 1;
}

.card-title {
  font-size: 0.95rem;
  font-weight: 600;
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  margin-bottom: 6px;
}

.card-author {
  font-size: 0.8rem;
  color: var(--accent);
  margin-bottom: 10px;
}

.card-footer {
  display: flex;
  align-items: center;
  gap: 12px;
}

.rating {
  font-size: 0.85rem;
  font-weight: 600;
}

.lang {
  font-size: 0.75rem;
  color: var(--text-secondary);
  padding: 2px 8px;
  background: var(--badge-bg);
  border-radius: 6px;
}

.card-btn {
  flex-shrink: 0;
  align-self: center;
  padding: 10px 20px;
  background: linear-gradient(135deg, var(--accent-secondary), var(--accent));
  opacity: 0.85;
  border: none;
  border-radius: 12px;
  color: white;
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  z-index: 1;
}

.card-btn:hover {
  opacity: 1;
  transform: scale(1.05);
  box-shadow: var(--shadow-accent);
}

.card-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  transform: none;
}
</style>