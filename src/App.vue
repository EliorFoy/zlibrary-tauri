<script setup lang="ts">
import { ref } from "vue";
import SearchView from "./views/SearchView.vue";
import DownloadView from "./views/DownloadView.vue";
import type { BookInfo } from "./types";

const currentView = ref<"search" | "download">("search");
const searchQuery = ref("");
const downloadingBook = ref<BookInfo | null>(null);

function onStartDownload(book: BookInfo) {
  downloadingBook.value = book;
  currentView.value = "download";
}

function onBackToSearch() {
  currentView.value = "search";
}
</script>

<template>
  <div class="app-container">
    <header class="app-header">
      <div class="header-content">
        <h1 class="app-title" @click="currentView = 'search'">
          <span class="icon">📚</span>
          Z-Library NoProxy
        </h1>
        <span class="header-subtitle">IP 直连绕过访问</span>
      </div>
    </header>

    <main class="app-main">
      <SearchView
        v-if="currentView === 'search'"
        v-model:query="searchQuery"
        @start-download="onStartDownload"
      />
      <DownloadView
        v-else-if="currentView === 'download' && downloadingBook"
        :book="downloadingBook"
        @back="onBackToSearch"
      />
    </main>

    <footer class="app-footer">
      <span>基于 GPL-3.0 | 仅供学习研究使用</span>
    </footer>
  </div>
</template>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
}

.app-header {
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border);
  padding: 16px 24px;
  position: sticky;
  top: 0;
  z-index: 100;
}

.header-content {
  max-width: 1200px;
  margin: 0 auto;
  display: flex;
  align-items: center;
  gap: 12px;
}

.app-title {
  font-size: 1.4rem;
  font-weight: 700;
  color: var(--accent);
  cursor: pointer;
  user-select: none;
}

.icon {
  margin-right: 6px;
}

.header-subtitle {
  font-size: 0.8rem;
  color: var(--text-secondary);
  padding: 4px 10px;
  background: rgba(233, 69, 96, 0.1);
  border-radius: 12px;
}

.app-main {
  flex: 1;
  max-width: 1200px;
  width: 100%;
  margin: 0 auto;
  padding: 24px;
}

.app-footer {
  text-align: center;
  padding: 16px;
  color: var(--text-secondary);
  font-size: 0.8rem;
  border-top: 1px solid var(--border);
}
</style>