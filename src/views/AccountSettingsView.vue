<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { AccountInfo } from "../types";

const emit = defineEmits<{ back: [] }>();

type Tab = "list" | "login" | "register" | "autoreg";

const tab = ref<Tab>("list");
const accounts = ref<AccountInfo[]>([]);
const statusMsg = ref("");
const errorMsg = ref("");

const loginEmail = ref("");
const loginPassword = ref("");

const regEmail = ref("");
const regPassword = ref("");
const regName = ref("");
const regCode = ref("");
const regStep = ref<"email" | "code" | "submit">("email");

async function loadAccounts() {
  try {
    accounts.value = await invoke<AccountInfo[]>("list_accounts");
  } catch (e) {
    errorMsg.value = String(e);
  }
}

onMounted(loadAccounts);

async function doLogin() {
  if (!loginEmail.value || !loginPassword.value) return;
  statusMsg.value = "正在登录...";
  errorMsg.value = "";
  try {
    await invoke("manual_login", {
      email: loginEmail.value,
      password: loginPassword.value,
    });
    statusMsg.value = `✅ ${loginEmail.value} 登录成功`;
    loginEmail.value = "";
    loginPassword.value = "";
    await loadAccounts();
    tab.value = "list";
  } catch (e) {
    errorMsg.value = String(e);
    statusMsg.value = "";
  }
}

async function sendCode() {
  if (!regEmail.value || !regPassword.value) return;
  statusMsg.value = "正在发送验证码...";
  errorMsg.value = "";
  try {
    if (!regName.value) {
      regName.value = "User_" + Math.random().toString(36).slice(2, 8);
    }
    await invoke("send_registration_code", {
      email: regEmail.value,
      password: regPassword.value,
      name: regName.value,
    });
    statusMsg.value = "验证码已发送，请查收邮件";
    regStep.value = "code";
  } catch (e) {
    errorMsg.value = String(e);
    statusMsg.value = "";
  }
}

async function doRegister() {
  if (!regCode.value) return;
  statusMsg.value = "正在注册...";
  errorMsg.value = "";
  try {
    await invoke("manual_register", {
      email: regEmail.value,
      password: regPassword.value,
      name: regName.value,
      code: regCode.value,
    });
    statusMsg.value = `✅ ${regEmail.value} 注册成功`;
    regEmail.value = "";
    regPassword.value = "";
    regName.value = "";
    regCode.value = "";
    regStep.value = "email";
    await loadAccounts();
    tab.value = "list";
  } catch (e) {
    errorMsg.value = String(e);
    statusMsg.value = "";
  }
}

async function deleteAccount(id: number) {
  try {
    await invoke("delete_account", { id });
    await loadAccounts();
    statusMsg.value = "账号已删除";
  } catch (e) {
    errorMsg.value = String(e);
  }
}
</script>

<template>
  <div class="settings">
    <header class="settings-header">
      <button class="btn-back" @click="emit('back')">← 返回搜索</button>
      <h2>账号管理</h2>
    </header>

    <nav class="tabs">
      <button :class="{ active: tab === 'list' }" @click="tab = 'list'">已有账号</button>
      <button :class="{ active: tab === 'login' }" @click="tab = 'login'">登录</button>
      <button :class="{ active: tab === 'register' }" @click="tab = 'register'">手动注册</button>
    </nav>

    <div v-if="statusMsg" class="msg-success">{{ statusMsg }}</div>
    <div v-if="errorMsg" class="msg-error">{{ errorMsg }}</div>

    <!-- Account List -->
    <div v-if="tab === 'list'" class="tab-content">
      <div v-if="accounts.length === 0" class="empty">暂无账号</div>
      <div v-else class="account-list">
        <div v-for="acct in accounts" :key="acct.id" class="account-card">
          <div class="account-info">
            <span class="acct-email">{{ acct.email }}</span>
            <span class="acct-id">#{{ acct.user_id }}</span>
            <span class="acct-usage">剩余 {{ acct.usage_count }} 次</span>
          </div>
          <button class="btn-danger" @click="deleteAccount(acct.id)">删除</button>
        </div>
      </div>
    </div>

    <!-- Login -->
    <div v-if="tab === 'login'" class="tab-content">
      <div class="form-group">
        <label>邮箱</label>
        <input v-model="loginEmail" type="email" placeholder="输入邮箱地址" @keyup.enter="doLogin" />
      </div>
      <div class="form-group">
        <label>密码</label>
        <input v-model="loginPassword" type="password" placeholder="输入密码" @keyup.enter="doLogin" />
      </div>
      <button class="btn-primary" @click="doLogin">登录</button>
    </div>

    <!-- Register -->
    <div v-if="tab === 'register'" class="tab-content">
      <div v-if="regStep === 'email'">
        <div class="form-group">
          <label>邮箱</label>
          <input v-model="regEmail" type="email" placeholder="输入邮箱地址" />
        </div>
        <div class="form-group">
          <label>密码</label>
          <input v-model="regPassword" type="password" placeholder="输入密码" />
        </div>
        <div class="form-group">
          <label>用户名（可选）</label>
          <input v-model="regName" type="text" placeholder="留空将自动生成" />
        </div>
        <button class="btn-primary" @click="sendCode">发送验证码</button>
      </div>
      <div v-else>
        <div class="form-group">
          <label>验证码</label>
          <input v-model="regCode" type="text" placeholder="输入邮件中的验证码" @keyup.enter="doRegister" />
        </div>
        <div class="form-actions">
          <button class="btn-secondary" @click="regStep = 'email'">← 返回</button>
          <button class="btn-primary" @click="doRegister">提交注册</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  padding: 0;
}

.settings-header {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 24px;
}

.settings-header h2 {
  font-size: 1.3rem;
  color: var(--accent);
}

.btn-back {
  background: var(--bg-card);
  color: var(--text-primary);
  border: 1px solid var(--border);
  padding: 8px 16px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 0.9rem;
}

.btn-back:hover {
  background: var(--accent);
  color: white;
}

.tabs {
  display: flex;
  gap: 8px;
  margin-bottom: 24px;
}

.tabs button {
  background: var(--bg-card);
  color: var(--text-secondary);
  border: 1px solid var(--border);
  padding: 10px 20px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 0.9rem;
  transition: all 0.2s;
}

.tabs button.active {
  background: var(--accent);
  color: white;
  border-color: var(--accent);
}

.tabs button:hover:not(.active) {
  color: var(--text-primary);
  border-color: var(--accent);
}

.msg-success {
  background: rgba(76, 175, 80, 0.15);
  color: #4caf50;
  padding: 10px 16px;
  border-radius: 8px;
  margin-bottom: 16px;
  font-size: 0.9rem;
}

.msg-error {
  background: rgba(233, 69, 96, 0.15);
  color: var(--accent);
  padding: 10px 16px;
  border-radius: 8px;
  margin-bottom: 16px;
  font-size: 0.9rem;
}

.tab-content {
  min-height: 200px;
}

.empty {
  color: var(--text-secondary);
  text-align: center;
  padding: 40px 0;
}

.account-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.account-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--bg-card);
  border: 1px solid var(--border);
  padding: 12px 16px;
  border-radius: 8px;
}

.account-info {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 0.9rem;
}

.acct-email {
  color: var(--text-primary);
  font-weight: 500;
}

.acct-id {
  color: var(--text-secondary);
  font-size: 0.8rem;
}

.acct-usage {
  color: var(--text-secondary);
  font-size: 0.8rem;
  background: var(--badge-bg);
  padding: 2px 8px;
  border-radius: 4px;
}

.btn-danger {
  background: transparent;
  color: var(--accent);
  border: 1px solid var(--accent);
  padding: 6px 14px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.8rem;
}

.btn-danger:hover {
  background: var(--accent);
  color: white;
}

.form-group {
  margin-bottom: 16px;
}

.form-group label {
  display: block;
  font-size: 0.85rem;
  color: var(--text-secondary);
  margin-bottom: 6px;
}

.form-group input {
  width: 100%;
  padding: 10px 14px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  color: var(--text-primary);
  font-size: 0.95rem;
  outline: none;
  transition: border-color 0.2s;
}

.form-group input:focus {
  border-color: var(--accent);
}

.btn-primary {
  background: var(--accent);
  color: white;
  border: none;
  padding: 10px 24px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 0.95rem;
  font-weight: 600;
  transition: background 0.2s;
}

.btn-primary:hover {
  background: var(--accent-hover);
}

.btn-secondary {
  background: var(--bg-card);
  color: var(--text-primary);
  border: 1px solid var(--border);
  padding: 10px 24px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 0.95rem;
}

.form-actions {
  display: flex;
  gap: 12px;
  margin-top: 8px;
}
</style>
