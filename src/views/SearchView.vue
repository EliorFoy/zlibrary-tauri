<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import BookCard from "../components/BookCard.vue";
import type { BookInfo, SearchResult } from "../types";

const emit = defineEmits<{
  "start-download": [book: BookInfo];
}>();

const query = defineModel<string>("query", { default: "" });

const loading = ref(false);
const books = ref<BookInfo[]>([]);
const currentPage = ref(1);
const totalResults = ref(0);
const error = ref("");

async function doSearch(page: number = 1) {
  if (!query.value.trim()) return;

  loading.value = true;
  error.value = "";
  currentPage.value = page;

  try {
    const result = await invoke<SearchResult>("search_books", {
      query: query.value.trim(),
      page,
    });
    books.value = result.books;
    totalResults.value = result.total;
  } catch (e: any) {
    error.value = typeof e === "string" ? e : e?.message || "搜索失败";
    books.value = [];
  } finally {
    loading.value = false;
  }
}

function onSearch() {
  doSearch(1);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter") onSearch();
}
</script>

<template>
  <div class="search-view">
    <div class="search-hero">
      <h2 class="search-title">探索 Z-Library</h2>
      <p class="search-subtitle">全球最大的电子图书馆 · IP 直连访问</p>
      <div class="search-box">
        <div class="search-input-wrap">
          <span class="search-icon">🔍</span>
          <input
            v-model="query"
            type="text"
            class="search-input"
            placeholder="输入书名、作者、ISBN 搜索…"
            @keydown="onKeydown"
          />
        </div>
        <button class="search-btn" :disabled="loading" @click="onSearch">
          <span v-if="loading" class="spinner"></span>
          <span v-else>搜索</span>
        </button>
      </div>
    </div>

    <div v-if="error" class="error-msg">{{ error }}</div>

    <div v-if="books.length > 0" class="results-section">
      <div class="results-header">
        <span>找到 {{ totalResults }} 个结果</span>
      </div>
      <div class="book-grid">
        <BookCard
          v-for="book in books"
          :key="book.id"
          :book="book"
          @download="emit('start-download', $event)"
        />
      </div>
      <div v-if="totalResults > books.length" class="pagination">
        <button
          class="page-btn"
          :disabled="currentPage <= 1"
          @click="doSearch(currentPage - 1)"
        >
          上一页
        </button>
        <span class="page-info">第 {{ currentPage }} 页</span>
        <button class="page-btn" @click="doSearch(currentPage + 1)">
          下一页
        </button>
      </div>
    </div>

    <div v-else-if="!loading && !error" class="empty-state">
      <span class="empty-icon">📖</span>
      <p>搜索你想要的书籍</p>
    </div>
  </div>
</template>

<style scoped>
.search-view {
  width: 100%;
}

.search-hero {
  text-align: center;
  padding: 48px 24px 36px;
  background: linear-gradient(135deg, #1a1a2e 0%, #16213e 40%, #7c3aed15 100%);
  border-radius: 24px;
  margin-bottom: 32px;
  border: 1px solid #7c3aed20;
}

.search-title {
  font-size: 2rem;
  font-weight: 800;
  background: linear-gradient(135deg, #7c4dff, #e94560);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  margin-bottom: 8px;
}

.search-subtitle {
  color: var(--text-secondary);
  font-size: 0.95rem;
  margin-bottom: 28px;
}

.search-box {
  display: flex;
  gap: 12px;
  max-width: 600px;
  margin: 0 auto;
}

.search-input-wrap {
  flex: 1;
  display: flex;
  align-items: center;
  background: #0a0a1a;
  border: 2px solid #2a2a4a;
  border-radius: 16px;
  padding: 0 16px;
  transition: border-color 0.3s;
}

.search-input-wrap:focus-within {
  border-color: #7c4dff;
}

.search-icon {
  font-size: 1.1rem;
  margin-right: 10px;
  opacity: 0.5;
}

.search-input {
  flex: 1;
  background: none;
  border: none;
  outline: none;
  color: var(--text-primary);
  font-size: 1rem;
  padding: 14px 0;
}

.search-btn {
  padding: 14px 32px;
  background: linear-gradient(135deg, #7c4dff, #e94560);
  border: none;
  border-radius: 16px;
  color: white;
  font-size: 1rem;
  font-weight: 600;
  cursor: pointer;
  transition: transform 0.2s, box-shadow 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 100px;
}

.search-btn:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 8px 24px rgba(124, 77, 255, 0.35);
}

.search-btn:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}

.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.error-msg {
  background: rgba(233, 69, 96, 0.1);
  border: 1px solid rgba(233, 69, 96, 0.3);
  color: var(--accent);
  padding: 14px 20px;
  border-radius: 12px;
  margin-bottom: 20px;
  text-align: center;
}

.results-section {
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

.results-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
  color: var(--text-secondary);
  font-size: 0.9rem;
}

.book-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 16px;
}

.pagination {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 16px;
  margin-top: 32px;
  padding: 20px 0;
}

.page-btn {
  padding: 10px 24px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  color: var(--text-primary);
  cursor: pointer;
  font-size: 0.9rem;
  transition: all 0.2s;
}

.page-btn:hover:not(:disabled) {
  border-color: #7c4dff;
  background: #7c4dff15;
}

.page-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.page-info {
  color: var(--text-secondary);
  font-size: 0.9rem;
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
</style>