import { invoke } from "@tauri-apps/api/core";

let SERVER_URL =
  localStorage.getItem("retlister_server_url") || "https://api.faky.dev";

export function setServerUrl(url) {
  SERVER_URL = url.replace(/\/$/, "");
  localStorage.setItem("retlister_server_url", SERVER_URL);
}

export function getServerUrl() {
  return SERVER_URL;
}

async function apiCall(method, endpoint, body = null) {
  const url = `${SERVER_URL}${endpoint}`;

  try {
    const responseString = await invoke("authenticated_request", {
      method,
      url,
      body,
    });
    return JSON.parse(responseString);
  } catch (error) {
    console.error(`API Error (${endpoint}):`, error);
    throw error;
  }
}

export async function listRestos() {
  return apiCall("GET", "/list");
}

export async function addResto(payload) {
  return apiCall("POST", "/add", JSON.stringify(payload));
}

export async function removeResto(id) {
  return apiCall("DELETE", `/remove/${id}`);
}

export async function searchResto(params) {
  const q = new URLSearchParams(params).toString();
  return apiCall("GET", `/search?${q}`);
}

export async function updateResto(id, payload) {
  return apiCall("POST", `/update/${id}`, JSON.stringify(payload));
}

export async function getStats() {
  return apiCall("GET", "/stats");
}

// ===== VAN API =====

export async function listVans() {
  return apiCall("GET", "/vans");
}

export async function getVan(id) {
  return apiCall("GET", `/vans/${id}`);
}

export async function addVan(payload) {
  console.log("Adding van with payload:", payload);
  return apiCall("POST", "/vans", JSON.stringify(payload));
}

export async function updateVan(id, payload) {
  return apiCall("POST", `/vans/${id}`, JSON.stringify(payload));
}

export async function deleteVan(id) {
  return apiCall("DELETE", `/vans/${id}`);
}

// ===== OPTIMIZE API =====

export async function optimizeLoading(van_id, items) {
  return apiCall("POST", "/optimize", JSON.stringify({ van_id, items }));
}

export async function optimizeCuts(
  cuts,
  kerf_width_mm = 3,
  min_remainder_width_mm = 100,
  min_remainder_height_mm = 100
) {
  console.log("API Call - optimizeCuts with:", {
    cuts,
    kerf_width_mm,
    min_remainder_width_mm,
    min_remainder_height_mm,
  });

  const payload = {
    cuts,
    kerf_width_mm,
    min_remainder_width_mm,
    min_remainder_height_mm,
  };

  const result = await apiCall(
    "POST",
    "/optimize_cuts",
    JSON.stringify(payload)
  );
  console.log("API Response data:", result);
  return result;
}
