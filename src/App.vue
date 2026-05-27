<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import SearchView from "./views/SearchView.vue";
import DownloadsView from "./views/DownloadsView.vue";
import AccountSettingsView from "./views/AccountSettingsView.vue";
import { tasks } from "./stores/downloadStore";

const currentView = ref<"search" | "downloads" | "accounts">("search");
const isDark = ref(true);

const activeDownloads = computed(() =>
  tasks.filter((t) => t.status === "queued" || t.status === "downloading").length
);

onMounted(() => {
  const saved = localStorage.getItem("theme");
  if (saved === "light") {
    isDark.value = false;
    document.documentElement.setAttribute("data-theme", "light");
  }
});

function toggleTheme() {
  isDark.value = !isDark.value;
  document.documentElement.setAttribute(
    "data-theme",
    isDark.value ? "" : "light"
  );
  localStorage.setItem("theme", isDark.value ? "dark" : "light");
}
</script>

<template>
  <div class="app-container">
    <!-- Desktop header -->
    <header class="app-header">
      <div class="header-inner">
        <div class="header-left" @click="currentView = 'search'">
          <span class="logo-icon">📚</span>
          <span class="app-title">Z-Library NoProxy</span>
          <span class="subtitle">IP 直连</span>
        </div>
        <nav class="desktop-nav">
          <button
            :class="{ active: currentView === 'search' }"
            @click="currentView = 'search'"
          >
            🔍 搜索
          </button>
          <button
            :class="{ active: currentView === 'downloads' }"
            @click="currentView = 'downloads'"
          >
            ⬇ 下载
            <span v-if="activeDownloads > 0" class="badge-dl">{{ activeDownloads }}</span>
          </button>
          <button
            :class="{ active: currentView === 'accounts' }"
            @click="currentView = 'accounts'"
          >
            👤 账号
          </button>
          <button class="theme-btn" @click="toggleTheme">
            {{ isDark ? "☀️" : "🌙" }}
          </button>
        </nav>
      </div>
    </header>

    <!-- Main content -->
    <main class="app-main">
      <div v-show="currentView === 'search'" class="view-wrap">
        <SearchView
          @start-download="currentView = 'downloads'"
        />
      </div>
      <div v-show="currentView === 'downloads'" class="view-wrap">
        <DownloadsView
          @start-search="currentView = 'search'"
        />
      </div>
      <div v-show="currentView === 'accounts'" class="view-wrap">
        <AccountSettingsView
          @back="currentView = 'search'"
        />
      </div>
    </main>

    <!-- Mobile bottom tabbar -->
    <nav class="mobile-tabbar">
      <button
        :class="{ active: currentView === 'search' }"
        @click="currentView = 'search'"
      >
        <span class="tab-icon">🔍</span>
        <span class="tab-label">搜索</span>
      </button>
      <button
        :class="{ active: currentView === 'downloads' }"
        @click="currentView = 'downloads'"
      >
        <span class="tab-icon">
          ⬇
          <span v-if="activeDownloads > 0" class="badge-dl-sm">{{ activeDownloads }}</span>
        </span>
        <span class="tab-label">下载</span>
      </button>
      <button
        :class="{ active: currentView === 'accounts' }"
        @click="currentView = 'accounts'"
      >
        <span class="tab-icon">👤</span>
        <span class="tab-label">账号</span>
      </button>
    </nav>

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

/* ========= Header (desktop) ========= */
.app-header {
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border);
  padding: 0 24px;
  position: sticky;
  top: 0;
  z-index: 100;
}

.header-inner {
  max-width: 1200px;
  margin: 0 auto;
  display: flex;
  align-items: center;
  height: 56px;
  gap: 16px;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  user-select: none;
  flex-shrink: 0;
}

.logo-icon {
  font-size: 1.4rem;
}

.app-title {
  font-size: 1.2rem;
  font-weight: 700;
  color: var(--accent);
}

.subtitle {
  font-size: 0.7rem;
  color: var(--text-secondary);
  padding: 2px 8px;
  background: var(--badge-bg);
  border-radius: 10px;
}

.desktop-nav {
  margin-left: auto;
  display: flex;
  gap: 6px;
}

.desktop-nav button {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid transparent;
  padding: 6px 14px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 0.85rem;
  transition: all 0.2s;
  position: relative;
}

.desktop-nav button.active {
  background: var(--accent);
  color: white;
  border-color: var(--accent);
}

.desktop-nav button:hover:not(.active) {
  color: var(--text-primary);
  border-color: var(--border);
}

.theme-btn {
  font-size: 1rem !important;
  padding: 6px 10px !important;
}

.badge-dl {
  position: absolute;
  top: -4px;
  right: -4px;
  background: var(--accent);
  color: white;
  font-size: 0.65rem;
  min-width: 16px;
  height: 16px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 4px;
  font-weight: 700;
}

/* ========= Main ========= */
.app-main {
  flex: 1;
  max-width: 1200px;
  width: 100%;
  margin: 0 auto;
  padding: 24px;
  padding-bottom: 80px;
}

.view-wrap {
  width: 100%;
}

/* ========= Mobile tabbar ========= */
.mobile-tabbar {
  display: none;
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  background: var(--bg-secondary);
  border-top: 1px solid var(--border);
  z-index: 200;
  padding: 6px 0;
  padding-bottom: max(6px, env(safe-area-inset-bottom));
}

.mobile-tabbar button {
  flex: 1;
  background: none;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 4px 0;
  transition: color 0.2s;
  position: relative;
}

.mobile-tabbar button.active {
  color: var(--accent);
}

.tab-icon {
  font-size: 1.3rem;
  position: relative;
}

.tab-label {
  font-size: 0.65rem;
}

.badge-dl-sm {
  position: absolute;
  top: -6px;
  right: -10px;
  background: var(--accent);
  color: white;
  font-size: 0.55rem;
  min-width: 14px;
  height: 14px;
  border-radius: 7px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 3px;
  font-weight: 700;
}

/* ========= Footer ========= */
.app-footer {
  text-align: center;
  padding: 12px;
  color: var(--text-secondary);
  font-size: 0.75rem;
  border-top: 1px solid var(--border);
}

/* ========= Responsive ========= */
@media (max-width: 768px) {
  .app-header {
    padding: 0 12px;
  }

  .app-title {
    font-size: 1rem;
  }

  .subtitle {
    display: none;
  }

  .desktop-nav {
    display: none;
  }

  .mobile-tabbar {
    display: flex;
  }

  .app-main {
    padding: 12px;
    padding-bottom: 70px;
  }

  .app-footer {
    display: none;
  }
}

@media (min-width: 769px) {
  .mobile-tabbar {
    display: none !important;
  }

  .app-main {
    padding-bottom: 24px;
  }
}
</style>
