/* Replacing file with a stable, tabbed implementation wired to proxy */
#define WIN32_LEAN_AND_MEAN
#define _WIN32_WINNT 0x0501
#include <windows.h>
#include <commctrl.h>
#include <wininet.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#pragma comment(lib, "comctl32.lib")
#pragma comment(lib, "wininet.lib")

/* Configuration */
#define PROXY_URL "http://192.168.56.1:8001"
#define WINDOW_WIDTH 1000
#define WINDOW_HEIGHT 600
#define MAX_RESTOS 1000

/* Control IDs */
#define IDC_TAB         1000
#define IDC_LISTVIEW    1001
#define IDC_BTN_ADD     1002
#define IDC_BTN_REMOVE  1003
#define IDC_BTN_REFRESH 1005
#define IDC_STATUSBAR   1006

/* Otimizar page */
#define IDC_ED_W        1101
#define IDC_ED_H        1102
#define IDC_ED_T        1103
#define IDC_ED_MAT      1104
#define IDC_BTN_FIND    1105
#define IDC_LISTSEARCH  1106

/* Estado page */
#define IDC_LBL_PROXY   1201
#define IDC_LBL_MAIN    1202
#define IDC_LBL_UPTIME  1203
#define IDC_LBL_DB      1204
#define IDC_LBL_PENDING 1205
#define IDC_BTN_STATUS  1206

/* Globals */
HINSTANCE g_hInstance;
HWND g_hMainWindow, g_hTab, g_hStatusBar;
HWND g_hListView; /* Retalhos */
HWND g_hEdW, g_hEdH, g_hEdT, g_hEdMat, g_hBtnFind, g_hListSearch; /* Otimizar */
HWND g_hLblProxy, g_hLblMain, g_hLblUptime, g_hLblDb, g_hLblPending, g_hBtnStatus; /* Estado */

char g_ProxyHost[128] = {0};
INTERNET_PORT g_ProxyPort = 80;

typedef struct {
    int id;
    int width_mm;
    int height_mm;
    int thickness_mm;
    char material[64];
    char notes[256];
    char created_at[32];
} Resto;

Resto g_inventory[MAX_RESTOS];
int g_inventoryCount = 0;

static void ParseProxyUrl(void) {
    const char* url = PROXY_URL;
    const char* p = strstr(url, "://");
    const char* host = p ? p + 3 : url;
    const char* colon = strchr(host, ':');
    const char* slash = strchr(host, '/');
    size_t hostLen;
    if (!colon) {
        hostLen = slash ? (size_t)(slash - host) : strlen(host);
        if (hostLen >= sizeof(g_ProxyHost)) hostLen = sizeof(g_ProxyHost) - 1;
        memcpy(g_ProxyHost, host, hostLen); g_ProxyHost[hostLen] = '\0';
        g_ProxyPort = 80;
    } else {
        hostLen = (size_t)(colon - host);
        if (hostLen >= sizeof(g_ProxyHost)) hostLen = sizeof(g_ProxyHost) - 1;
        memcpy(g_ProxyHost, host, hostLen); g_ProxyHost[hostLen] = '\0';
        g_ProxyPort = (INTERNET_PORT)atoi(colon + 1);
    }
}

static int HttpRequestEx(const char* method, const char* path, const char* jsonBody, char** outResponse) {
    char* result = (char*)malloc(4096);
    DWORD resultSize = 0, resultCapacity = 4096;
    HINTERNET hInternet = NULL, hConnect = NULL, hRequest = NULL;
    if (!result) return 0; result[0] = '\0';

    hInternet = InternetOpenA("RetLister/1.0", INTERNET_OPEN_TYPE_DIRECT, NULL, NULL, 0);
    if (!hInternet) { free(result); return 0; }
    hConnect = InternetConnectA(hInternet, g_ProxyHost, g_ProxyPort, NULL, NULL, INTERNET_SERVICE_HTTP, 0, 0);
    if (!hConnect) { InternetCloseHandle(hInternet); free(result); return 0; }
    DWORD flags = INTERNET_FLAG_RELOAD | INTERNET_FLAG_NO_CACHE_WRITE | INTERNET_FLAG_NO_COOKIES | INTERNET_FLAG_KEEP_CONNECTION;
    hRequest = HttpOpenRequestA(hConnect, method, path, NULL, NULL, NULL, flags, 0);
    if (!hRequest) { InternetCloseHandle(hConnect); InternetCloseHandle(hInternet); free(result); return 0; }
    const char* headers = "Content-Type: application/json\r\n";
    BOOL ok;
    if (jsonBody && (strcmp(method, "POST") == 0 || strcmp(method, "PUT") == 0))
        ok = HttpSendRequestA(hRequest, headers, (DWORD)strlen(headers), (LPVOID)jsonBody, (DWORD)strlen(jsonBody));
    else
        ok = HttpSendRequestA(hRequest, headers, (DWORD)strlen(headers), NULL, 0);
    if (!ok) { InternetCloseHandle(hRequest); InternetCloseHandle(hConnect); InternetCloseHandle(hInternet); free(result); return 0; }
    for (;;) {
        char buffer[4096]; DWORD bytesRead = 0;
        if (!InternetReadFile(hRequest, buffer, sizeof(buffer), &bytesRead) || bytesRead == 0) break;
        if (resultSize + bytesRead >= resultCapacity) {
            resultCapacity *= 2; char* nr = (char*)realloc(result, resultCapacity); if (!nr) break; result = nr;
        }
        memcpy(result + resultSize, buffer, bytesRead); resultSize += bytesRead;
    }
    result[resultSize] = '\0';
    InternetCloseHandle(hRequest); InternetCloseHandle(hConnect); InternetCloseHandle(hInternet);
    if (outResponse) *outResponse = result; else free(result);
    return 1;
}

/* Simple JSON parser */
void ParseRestoList(const char* json) {
    const char* p = json;
    g_inventoryCount = 0;
    
    while (*p && g_inventoryCount < MAX_RESTOS) {
        p = strstr(p, "{\"id\":");
        if (!p) break;
        
        Resto* r = &g_inventory[g_inventoryCount];
        memset(r, 0, sizeof(Resto));
        
        /* Parse id */
        if (sscanf(p, "{\"id\":%d", &r->id) == 1) {
            /* Parse width_mm */
            const char* w = strstr(p, "\"width_mm\":");
            if (w) sscanf(w, "\"width_mm\":%d", &r->width_mm);
            
            /* Parse height_mm */
            const char* h = strstr(p, "\"height_mm\":");
            if (h) sscanf(h, "\"height_mm\":%d", &r->height_mm);
            
            /* Parse thickness_mm */
            const char* t = strstr(p, "\"thickness_mm\":");
            if (t) sscanf(t, "\"thickness_mm\":%d", &r->thickness_mm);
            
            /* Parse material */
            const char* m = strstr(p, "\"material\":\"");
            if (m) {
                m += 12;
                const char* end = strchr(m, '"');
                if (end) {
                    int len = (int)(end - m);
                    if (len >= sizeof(r->material)) len = sizeof(r->material) - 1;
                    memcpy(r->material, m, len);
                    r->material[len] = '\0';
                }
            }
            
            /* Parse notes */
            const char* n = strstr(p, "\"notes\":");
            if (n) {
                n += 8;
                if (strncmp(n, "null", 4) != 0 && *n == '"') {
                    n++;
                    const char* end = strchr(n, '"');
                    if (end) {
                        int len = (int)(end - n);
                        if (len >= sizeof(r->notes)) len = sizeof(r->notes) - 1;
                        memcpy(r->notes, n, len);
                        r->notes[len] = '\0';
                    }
                }
            }
            
            g_inventoryCount++;
        }
        
        p++;
    }
}

void RefreshListView(void) {
    char url[256];
    char status[256];
    int i;
    
    ListView_DeleteAllItems(g_hListView);
    
    sprintf(url, "/list");
    char* response = NULL;
    HttpRequestEx("GET", url, NULL, &response);
    
    if (response) {
        ParseRestoList(response);
        free(response);
    }
    
    for (i = 0; i < g_inventoryCount; i++) {
        LVITEMA lvi;
        char buf[256];
        int idx;
        
        memset(&lvi, 0, sizeof(lvi));
        lvi.mask = LVIF_TEXT;
        lvi.iItem = i;
        
        /* ID column */
        sprintf(buf, "#%d", g_inventory[i].id);
        lvi.pszText = buf;
        lvi.iSubItem = 0;
        idx = ListView_InsertItem(g_hListView, &lvi);
        
        /* Width */
        sprintf(buf, "%d", g_inventory[i].width_mm);
        ListView_SetItemText(g_hListView, idx, 1, buf);
        
        /* Height */
        sprintf(buf, "%d", g_inventory[i].height_mm);
        ListView_SetItemText(g_hListView, idx, 2, buf);
        
        /* Thickness */
        sprintf(buf, "%d", g_inventory[i].thickness_mm);
        ListView_SetItemText(g_hListView, idx, 3, buf);
        
        /* Material */
        ListView_SetItemText(g_hListView, idx, 4, g_inventory[i].material);
        
        /* Notes */
        ListView_SetItemText(g_hListView, idx, 5, g_inventory[i].notes);
    }
    
    sprintf(status, "%d retalhos", g_inventoryCount);
    SendMessage(g_hStatusBar, SB_SETTEXT, 0, (LPARAM)status);
}

void CreateListView(HWND hwndParent) {
    LVCOLUMNA lvc;
    
    g_hListView = CreateWindowEx(
        WS_EX_CLIENTEDGE,
        WC_LISTVIEW,
        "",
        WS_CHILD | WS_VISIBLE | LVS_REPORT | LVS_SINGLESEL | WS_BORDER,
        10, 50, WINDOW_WIDTH - 30, WINDOW_HEIGHT - 120,
        hwndParent,
        (HMENU)IDC_LISTVIEW,
        g_hInstance,
        NULL
    );
    
    ListView_SetExtendedListViewStyle(g_hListView, LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES);
    
    memset(&lvc, 0, sizeof(lvc));
    lvc.mask = LVCF_TEXT | LVCF_WIDTH;
    
    lvc.pszText = "ID";
    lvc.cx = 60;
    ListView_InsertColumn(g_hListView, 0, &lvc);
    
    lvc.pszText = "Largura";
    lvc.cx = 100;
    ListView_InsertColumn(g_hListView, 1, &lvc);
    
    lvc.pszText = "Altura";
    lvc.cx = 100;
    ListView_InsertColumn(g_hListView, 2, &lvc);
    
    lvc.pszText = "Espessura";
    lvc.cx = 100;
    ListView_InsertColumn(g_hListView, 3, &lvc);
    
    lvc.pszText = "Material";
    lvc.cx = 120;
    ListView_InsertColumn(g_hListView, 4, &lvc);
    
    lvc.pszText = "Notas";
    lvc.cx = 400;
    ListView_InsertColumn(g_hListView, 5, &lvc);
}

void RemoveSelected(void) {
    int selected = ListView_GetNextItem(g_hListView, -1, LVNI_SELECTED);
    char msg[256];
    char url[512];
    int id;
    
    if (selected < 0 || selected >= g_inventoryCount) {
        MessageBox(g_hMainWindow, "Sem selecao", "Remover", MB_OK | MB_ICONWARNING);
        return;
    }
    
    id = g_inventory[selected].id;
    
    sprintf(msg, "Remover resto #%d?", id);
    if (MessageBox(g_hMainWindow, msg, "Confirmar", MB_YESNO | MB_ICONQUESTION) != IDYES) {
        return;
    }
    
    sprintf(url, "/remove/%d", id);
    if (HttpRequestEx("DELETE", url, NULL, NULL)) {
        RefreshListView();
    } else {
        MessageBox(g_hMainWindow, "Erro ao remover", "Erro", MB_OK | MB_ICONERROR);
    }
}

static void CreateRetalhosPage(HWND hwndParent) {
    CreateWindow("BUTTON", "Adicionar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        10, 10, 100, 30, hwndParent, (HMENU)IDC_BTN_ADD, g_hInstance, NULL);

    CreateWindow("BUTTON", "Remover", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        120, 10, 100, 30, hwndParent, (HMENU)IDC_BTN_REMOVE, g_hInstance, NULL);

    CreateWindow("BUTTON", "Atualizar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        230, 10, 100, 30, hwndParent, (HMENU)IDC_BTN_REFRESH, g_hInstance, NULL);

    CreateListView(hwndParent);
}

static void CreateOtimizarPage(HWND hwndParent) {
    CreateWindow("STATIC", "Largura (mm):", WS_CHILD | WS_VISIBLE,
        10, 12, 90, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdW = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER,
        105, 10, 80, 22, hwndParent, (HMENU)IDC_ED_W, g_hInstance, NULL);

    CreateWindow("STATIC", "Altura (mm):", WS_CHILD | WS_VISIBLE,
        200, 12, 80, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdH = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER,
        285, 10, 80, 22, hwndParent, (HMENU)IDC_ED_H, g_hInstance, NULL);

    CreateWindow("STATIC", "Espessura:", WS_CHILD | WS_VISIBLE,
        380, 12, 70, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdT = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER,
        455, 10, 60, 22, hwndParent, (HMENU)IDC_ED_T, g_hInstance, NULL);

    CreateWindow("STATIC", "Material:", WS_CHILD | WS_VISIBLE,
        530, 12, 60, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdMat = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER,
        595, 10, 120, 22, hwndParent, (HMENU)IDC_ED_MAT, g_hInstance, NULL);

    g_hBtnFind = CreateWindow("BUTTON", "Procurar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        720, 10, 90, 24, hwndParent, (HMENU)IDC_BTN_FIND, g_hInstance, NULL);

    g_hListSearch = CreateWindowEx(
        WS_EX_CLIENTEDGE,
        WC_LISTVIEW,
        "",
        WS_CHILD | WS_VISIBLE | LVS_REPORT | WS_BORDER,
        10, 45, WINDOW_WIDTH - 30, WINDOW_HEIGHT - 120,
        hwndParent,
        (HMENU)IDC_LISTSEARCH,
        g_hInstance,
        NULL
    );
    ListView_SetExtendedListViewStyle(g_hListSearch, LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES);

    LVCOLUMNA lvc; memset(&lvc, 0, sizeof(lvc)); lvc.mask = LVCF_TEXT | LVCF_WIDTH;
    lvc.pszText = "ID"; lvc.cx = 60; ListView_InsertColumn(g_hListSearch, 0, &lvc);
    lvc.pszText = "Largura"; lvc.cx = 100; ListView_InsertColumn(g_hListSearch, 1, &lvc);
    lvc.pszText = "Altura"; lvc.cx = 100; ListView_InsertColumn(g_hListSearch, 2, &lvc);
    lvc.pszText = "Espessura"; lvc.cx = 100; ListView_InsertColumn(g_hListSearch, 3, &lvc);
    lvc.pszText = "Material"; lvc.cx = 120; ListView_InsertColumn(g_hListSearch, 4, &lvc);
    lvc.pszText = "Notas"; lvc.cx = 400; ListView_InsertColumn(g_hListSearch, 5, &lvc);
}

static void CreateEstadoPage(HWND hwndParent) {
    CreateWindow("STATIC", "Proxy:", WS_CHILD | WS_VISIBLE, 10, 14, 50, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblProxy = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, 70, 14, 200, 18, hwndParent, (HMENU)IDC_LBL_PROXY, g_hInstance, NULL);

    CreateWindow("STATIC", "Main server:", WS_CHILD | WS_VISIBLE, 10, 38, 80, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblMain = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, 100, 38, 200, 18, hwndParent, (HMENU)IDC_LBL_MAIN, g_hInstance, NULL);

    CreateWindow("STATIC", "Uptime:", WS_CHILD | WS_VISIBLE, 10, 62, 60, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblUptime = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, 80, 62, 200, 18, hwndParent, (HMENU)IDC_LBL_UPTIME, g_hInstance, NULL);

    CreateWindow("STATIC", "DB:", WS_CHILD | WS_VISIBLE, 10, 86, 40, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblDb = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, 60, 86, 400, 18, hwndParent, (HMENU)IDC_LBL_DB, g_hInstance, NULL);

    CreateWindow("STATIC", "Pendentes:", WS_CHILD | WS_VISIBLE, 10, 110, 70, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblPending = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, 90, 110, 200, 18, hwndParent, (HMENU)IDC_LBL_PENDING, g_hInstance, NULL);

    g_hBtnStatus = CreateWindow("BUTTON", "Atualizar estado", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        10, 140, 130, 24, hwndParent, (HMENU)IDC_BTN_STATUS, g_hInstance, NULL);
}

static void SwitchTab(int index) {
    /* Show/Hide per tab */
    BOOL showRet = (index == 0), showOpt = (index == 1), showSt = (index == 2);
    /* Retalhos */
    ShowWindow(GetDlgItem(g_hMainWindow, IDC_BTN_ADD), showRet ? SW_SHOW : SW_HIDE);
    ShowWindow(GetDlgItem(g_hMainWindow, IDC_BTN_REMOVE), showRet ? SW_SHOW : SW_HIDE);
    ShowWindow(GetDlgItem(g_hMainWindow, IDC_BTN_REFRESH), showRet ? SW_SHOW : SW_HIDE);
    ShowWindow(g_hListView, showRet ? SW_SHOW : SW_HIDE);
    /* Otimizar */
    ShowWindow(g_hEdW, showOpt ? SW_SHOW : SW_HIDE);
    ShowWindow(g_hEdH, showOpt ? SW_SHOW : SW_HIDE);
    ShowWindow(g_hEdT, showOpt ? SW_SHOW : SW_HIDE);
    ShowWindow(g_hEdMat, showOpt ? SW_SHOW : SW_HIDE);
    ShowWindow(g_hBtnFind, showOpt ? SW_SHOW : SW_HIDE);
    ShowWindow(g_hListSearch, showOpt ? SW_SHOW : SW_HIDE);
    /* Labels on Otimizar */
    /* they are static controls without IDs; toggle by walking siblings would be noisy; skip */
    /* Estado */
    ShowWindow(g_hLblProxy, showSt ? SW_SHOW : SW_HIDE);
    ShowWindow(g_hLblMain, showSt ? SW_SHOW : SW_HIDE);
    ShowWindow(g_hLblUptime, showSt ? SW_SHOW : SW_HIDE);
    ShowWindow(g_hLblDb, showSt ? SW_SHOW : SW_HIDE);
    ShowWindow(g_hLblPending, showSt ? SW_SHOW : SW_HIDE);
    ShowWindow(g_hBtnStatus, showSt ? SW_SHOW : SW_HIDE);
}

static void DoSearch(void) {
    char wbuf[16], hbuf[16], tbuf[16], mat[64];
    GetWindowTextA(g_hEdW, wbuf, sizeof(wbuf));
    GetWindowTextA(g_hEdH, hbuf, sizeof(hbuf));
    GetWindowTextA(g_hEdT, tbuf, sizeof(tbuf));
    GetWindowTextA(g_hEdMat, mat, sizeof(mat));
    if (!wbuf[0] || !hbuf[0] || !tbuf[0] || !mat[0]) {
        MessageBox(g_hMainWindow, "Preencha todos os campos", "Pesquisar", MB_OK | MB_ICONWARNING);
        return;
    }
    char json[256];
    wsprintfA(json, "{\"width_mm\":%d,\"height_mm\":%d,\"thickness_mm\":%d,\"material\":\"%s\"}", atoi(wbuf), atoi(hbuf), atoi(tbuf), mat);
    char* resp = NULL;
    if (!HttpRequestEx("POST", "/search", json, &resp)) {
        MessageBox(g_hMainWindow, "Falha na requisicao", "Erro", MB_OK | MB_ICONERROR);
        return;
    }
    /* Reuse parser and fill g_hListSearch */
    ListView_DeleteAllItems(g_hListSearch);
    ParseRestoList(resp);
    free(resp);
    for (int i = 0; i < g_inventoryCount; i++) {
        LVITEMA lvi; char buf[256]; int idx;
        ZeroMemory(&lvi, sizeof(lvi)); lvi.mask = LVIF_TEXT; lvi.iItem = i;
        wsprintfA(buf, "#%d", g_inventory[i].id); lvi.pszText = buf; lvi.iSubItem = 0; idx = ListView_InsertItem(g_hListSearch, &lvi);
        wsprintfA(buf, "%d", g_inventory[i].width_mm); ListView_SetItemText(g_hListSearch, idx, 1, buf);
        wsprintfA(buf, "%d", g_inventory[i].height_mm); ListView_SetItemText(g_hListSearch, idx, 2, buf);
        wsprintfA(buf, "%d", g_inventory[i].thickness_mm); ListView_SetItemText(g_hListSearch, idx, 3, buf);
        ListView_SetItemText(g_hListSearch, idx, 4, g_inventory[i].material);
        ListView_SetItemText(g_hListSearch, idx, 5, g_inventory[i].notes);
    }
}

static void LoadStatus(void) {
    char* resp = NULL; char buf[256];
    if (HttpRequestEx("GET", "/health", NULL, &resp)) {
        /* naive parse */
        const char* p1 = strstr(resp, "\"proxy_active\":");
        const char* p2 = strstr(resp, "\"main_server_active\":");
        const char* p3 = strstr(resp, "\"uptime_seconds\":");
        const char* p4 = strstr(resp, "\"db_path\":\"");
        if (p1) { BOOL on = (strstr(p1, "true") != NULL); SetWindowTextA(g_hLblProxy, on ? "Ligado" : "Desligado"); }
        if (p2) { BOOL on = (strstr(p2, "true") != NULL); SetWindowTextA(g_hLblMain, on ? "Ligado" : "Desligado"); }
        if (p3) { unsigned long secs = 0; sscanf(p3, "\"uptime_seconds\":%lu", &secs); wsprintfA(buf, "%lu s", secs); SetWindowTextA(g_hLblUptime, buf); }
        if (p4) { p4 += 13; const char* e = strchr(p4, '"'); if (e) { size_t n = e - p4; if (n > sizeof(buf)-1) n = sizeof(buf)-1; memcpy(buf, p4, n); buf[n]='\0'; SetWindowTextA(g_hLblDb, buf);} }
        free(resp);
    }
    resp = NULL;
    if (HttpRequestEx("GET", "/sync/status", NULL, &resp)) {
        const char* p = strstr(resp, "\"pending_changes\":");
        if (p) { int pend = 0; sscanf(p, "\"pending_changes\":%d", &pend); wsprintfA(buf, "%d", pend); SetWindowTextA(g_hLblPending, buf); }
        free(resp);
    }
}

static void CreateRetalhosPage(HWND hwndParent) {
    CreateWindow("BUTTON", "Adicionar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        10, 10, 100, 30, hwndParent, (HMENU)IDC_BTN_ADD, g_hInstance, NULL);
    CreateWindow("BUTTON", "Remover", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        120, 10, 100, 30, hwndParent, (HMENU)IDC_BTN_REMOVE, g_hInstance, NULL);
    CreateWindow("BUTTON", "Atualizar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        230, 10, 100, 30, hwndParent, (HMENU)IDC_BTN_REFRESH, g_hInstance, NULL);
    CreateListView(hwndParent);
}

static void CreateOtimizarPage(HWND hwndParent) {
    CreateWindow("STATIC", "Largura (mm):", WS_CHILD | WS_VISIBLE, 10, 12, 90, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdW = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, 105, 10, 80, 22, hwndParent, (HMENU)IDC_ED_W, g_hInstance, NULL);
    CreateWindow("STATIC", "Altura (mm):", WS_CHILD | WS_VISIBLE, 200, 12, 80, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdH = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, 285, 10, 80, 22, hwndParent, (HMENU)IDC_ED_H, g_hInstance, NULL);
    CreateWindow("STATIC", "Espessura:", WS_CHILD | WS_VISIBLE, 380, 12, 70, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdT = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, 455, 10, 60, 22, hwndParent, (HMENU)IDC_ED_T, g_hInstance, NULL);
    CreateWindow("STATIC", "Material:", WS_CHILD | WS_VISIBLE, 530, 12, 60, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdMat = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER, 595, 10, 120, 22, hwndParent, (HMENU)IDC_ED_MAT, g_hInstance, NULL);
    g_hBtnFind = CreateWindow("BUTTON", "Procurar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON, 720, 10, 90, 24, hwndParent, (HMENU)IDC_BTN_FIND, g_hInstance, NULL);
    g_hListSearch = CreateWindowEx(WS_EX_CLIENTEDGE, WC_LISTVIEW, "", WS_CHILD | WS_VISIBLE | LVS_REPORT | WS_BORDER,
        10, 45, WINDOW_WIDTH - 30, WINDOW_HEIGHT - 120, hwndParent, (HMENU)IDC_LISTSEARCH, g_hInstance, NULL);
    ListView_SetExtendedListViewStyle(g_hListSearch, LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES);
    LVCOLUMNA lvc; ZeroMemory(&lvc, sizeof(lvc)); lvc.mask = LVCF_TEXT | LVCF_WIDTH;
    lvc.pszText = "ID"; lvc.cx = 60; ListView_InsertColumn(g_hListSearch, 0, &lvc);
    lvc.pszText = "Largura"; lvc.cx = 100; ListView_InsertColumn(g_hListSearch, 1, &lvc);
    lvc.pszText = "Altura"; lvc.cx = 100; ListView_InsertColumn(g_hListSearch, 2, &lvc);
    lvc.pszText = "Espessura"; lvc.cx = 100; ListView_InsertColumn(g_hListSearch, 3, &lvc);
    lvc.pszText = "Material"; lvc.cx = 120; ListView_InsertColumn(g_hListSearch, 4, &lvc);
    lvc.pszText = "Notas"; lvc.cx = 400; ListView_InsertColumn(g_hListSearch, 5, &lvc);
}

static void CreateEstadoPage(HWND hwndParent) {
    CreateWindow("STATIC", "Proxy:", WS_CHILD | WS_VISIBLE, 10, 14, 50, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblProxy = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, 70, 14, 200, 18, hwndParent, (HMENU)IDC_LBL_PROXY, g_hInstance, NULL);
    CreateWindow("STATIC", "Main server:", WS_CHILD | WS_VISIBLE, 10, 38, 80, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblMain = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, 100, 38, 200, 18, hwndParent, (HMENU)IDC_LBL_MAIN, g_hInstance, NULL);
    CreateWindow("STATIC", "Uptime:", WS_CHILD | WS_VISIBLE, 10, 62, 60, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblUptime = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, 80, 62, 200, 18, hwndParent, (HMENU)IDC_LBL_UPTIME, g_hInstance, NULL);
    CreateWindow("STATIC", "DB:", WS_CHILD | WS_VISIBLE, 10, 86, 40, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblDb = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, 60, 86, 400, 18, hwndParent, (HMENU)IDC_LBL_DB, g_hInstance, NULL);
    CreateWindow("STATIC", "Pendentes:", WS_CHILD | WS_VISIBLE, 10, 110, 70, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblPending = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, 90, 110, 200, 18, hwndParent, (HMENU)IDC_LBL_PENDING, g_hInstance, NULL);
    g_hBtnStatus = CreateWindow("BUTTON", "Atualizar estado", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON, 10, 140, 130, 24, hwndParent, (HMENU)IDC_BTN_STATUS, g_hInstance, NULL);
}

static void SwitchTab(int idx) {
    BOOL showRet = (idx==0), showOpt=(idx==1), showSt=(idx==2);
    ShowWindow(GetDlgItem(g_hMainWindow, IDC_BTN_ADD), showRet?SW_SHOW:SW_HIDE);
    ShowWindow(GetDlgItem(g_hMainWindow, IDC_BTN_REMOVE), showRet?SW_SHOW:SW_HIDE);
    ShowWindow(GetDlgItem(g_hMainWindow, IDC_BTN_REFRESH), showRet?SW_SHOW:SW_HIDE);
    ShowWindow(g_hListView, showRet?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdW, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdH, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdT, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdMat, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hBtnFind, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hListSearch, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblProxy, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblMain, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblUptime, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblDb, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblPending, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hBtnStatus, showSt?SW_SHOW:SW_HIDE);
}

static void DoSearch(void) {
    char w[16], h[16], t[16], m[64];
    GetWindowTextA(g_hEdW, w, sizeof(w));
    GetWindowTextA(g_hEdH, h, sizeof(h));
    GetWindowTextA(g_hEdT, t, sizeof(t));
    GetWindowTextA(g_hEdMat, m, sizeof(m));
    if (!w[0]||!h[0]||!t[0]||!m[0]) { MessageBox(g_hMainWindow, "Preencha todos os campos", "Pesquisar", MB_OK|MB_ICONWARNING); return; }
    char json[256]; wsprintfA(json, "{\"width_mm\":%d,\"height_mm\":%d,\"thickness_mm\":%d,\"material\":\"%s\"}", atoi(w), atoi(h), atoi(t), m);
    char* resp=NULL; if (!HttpRequestEx("POST","/search",json,&resp)) { MessageBox(g_hMainWindow,"Falha na requisicao","Erro",MB_OK|MB_ICONERROR); return; }
    ListView_DeleteAllItems(g_hListSearch); ParseRestoList(resp); free(resp);
    for (int i=0;i<g_inventoryCount;i++){ LVITEMA lvi; char buf[64]; ZeroMemory(&lvi,sizeof(lvi)); lvi.mask=LVIF_TEXT; lvi.iItem=i; wsprintfA(buf,"#%d",g_inventory[i].id); lvi.pszText=buf; lvi.iSubItem=0; int idx=ListView_InsertItem(g_hListSearch,&lvi);
        wsprintfA(buf,"%d",g_inventory[i].width_mm); ListView_SetItemText(g_hListSearch,idx,1,buf);
        wsprintfA(buf,"%d",g_inventory[i].height_mm); ListView_SetItemText(g_hListSearch,idx,2,buf);
        wsprintfA(buf,"%d",g_inventory[i].thickness_mm); ListView_SetItemText(g_hListSearch,idx,3,buf);
        ListView_SetItemText(g_hListSearch,idx,4,g_inventory[i].material);
        ListView_SetItemText(g_hListSearch,idx,5,g_inventory[i].notes);
    }
}

static void LoadStatus(void) {
    char* resp=NULL; char buf[256];
    if (HttpRequestEx("GET","/health",NULL,&resp)){
        const char* p1=strstr(resp,"\"proxy_active\":"); BOOL on1=(p1&&strstr(p1,"true")); SetWindowTextA(g_hLblProxy,on1?"Ligado":"Desligado");
        const char* p2=strstr(resp,"\"main_server_active\":"); BOOL on2=(p2&&strstr(p2,"true")); SetWindowTextA(g_hLblMain,on2?"Ligado":"Desligado");
        const char* p3=strstr(resp,"\"uptime_seconds\":"); unsigned long s=0; if(p3) sscanf(p3,"\"uptime_seconds\":%lu",&s); wsprintfA(buf,"%lu s",s); SetWindowTextA(g_hLblUptime,buf);
        const char* p4=strstr(resp,"\"db_path\":\""); if(p4){ p4+=13; const char* e=strchr(p4,'"'); size_t n=e? (size_t)(e-p4):0; if(n>sizeof(buf)-1) n=sizeof(buf)-1; memcpy(buf,p4,n); buf[n]='\0'; SetWindowTextA(g_hLblDb,buf);} free(resp);
    }
    resp=NULL; if (HttpRequestEx("GET","/sync/status",NULL,&resp)){ const char* p=strstr(resp,"\"pending_changes\":"); int pend=0; if(p) sscanf(p,"\"pending_changes\":%d",&pend); wsprintfA(buf,"%d",pend); SetWindowTextA(g_hLblPending,buf); free(resp);} }

void RefreshListView(void);
void CreateListView(HWND hwndParent) {
    LVCOLUMNA lvc; ZeroMemory(&lvc, sizeof(lvc));
    g_hListView = CreateWindowEx(WS_EX_CLIENTEDGE, WC_LISTVIEW, "", WS_CHILD|WS_VISIBLE|LVS_REPORT|WS_BORDER,
        10, 50, WINDOW_WIDTH-30, WINDOW_HEIGHT-120, hwndParent, (HMENU)IDC_LISTVIEW, g_hInstance, NULL);
    ListView_SetExtendedListViewStyle(g_hListView, LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES);
    lvc.mask=LVCF_TEXT|LVCF_WIDTH; lvc.pszText="ID"; lvc.cx=60; ListView_InsertColumn(g_hListView,0,&lvc);
    lvc.pszText="Largura"; lvc.cx=100; ListView_InsertColumn(g_hListView,1,&lvc);
    lvc.pszText="Altura"; lvc.cx=100; ListView_InsertColumn(g_hListView,2,&lvc);
    lvc.pszText="Espessura"; lvc.cx=100; ListView_InsertColumn(g_hListView,3,&lvc);
    lvc.pszText="Material"; lvc.cx=120; ListView_InsertColumn(g_hListView,4,&lvc);
    lvc.pszText="Notas"; lvc.cx=400; ListView_InsertColumn(g_hListView,5,&lvc);
}

void RefreshListView(void) {
    ListView_DeleteAllItems(g_hListView);
    char* response=NULL; if (HttpRequestEx("GET","/list",NULL,&response)) { ParseRestoList(response); free(response);} 
    char status[64];
    for (int i=0;i<g_inventoryCount;i++){
        LVITEMA lvi; char buf[64]; int idx; ZeroMemory(&lvi,sizeof(lvi)); lvi.mask=LVIF_TEXT; lvi.iItem=i;
        wsprintfA(buf,"#%d",g_inventory[i].id); lvi.pszText=buf; lvi.iSubItem=0; idx=ListView_InsertItem(g_hListView,&lvi);
        wsprintfA(buf,"%d",g_inventory[i].width_mm); ListView_SetItemText(g_hListView,idx,1,buf);
        wsprintfA(buf,"%d",g_inventory[i].height_mm); ListView_SetItemText(g_hListView,idx,2,buf);
        wsprintfA(buf,"%d",g_inventory[i].thickness_mm); ListView_SetItemText(g_hListView,idx,3,buf);
        ListView_SetItemText(g_hListView,idx,4,g_inventory[i].material);
        ListView_SetItemText(g_hListView,idx,5,g_inventory[i].notes);
    }
    wsprintfA(status, "%d retalhos", g_inventoryCount); SendMessage(g_hStatusBar, SB_SETTEXT, 0, (LPARAM)status);
}

void RemoveSelected(void) {
    int sel = ListView_GetNextItem(g_hListView, -1, LVNI_SELECTED);
    if (sel<0 || sel>=g_inventoryCount){ MessageBox(g_hMainWindow, "Sem selecao", "Remover", MB_OK|MB_ICONWARNING); return; }
    int id = g_inventory[sel].id; char msg[64]; wsprintfA(msg, "Remover resto #%d?", id);
    if (IDYES != MessageBox(g_hMainWindow, msg, "Confirmar", MB_YESNO|MB_ICONQUESTION)) return;
    char path[64]; wsprintfA(path, "/remove/%d", id); if (!HttpRequestEx("DELETE", path, NULL, NULL)) { MessageBox(g_hMainWindow, "Erro ao remover", "Erro", MB_OK|MB_ICONERROR); return; }
    RefreshListView();
}

/* Add dialog */
static LRESULT CALLBACK AddWndProc(HWND hDlg, UINT uMsg, WPARAM wParam, LPARAM lParam) {
    static HWND eMat,eW,eH,eT,eNotes;
    switch(uMsg){
        case WM_CREATE:
            CreateWindow("STATIC","Material:",WS_CHILD|WS_VISIBLE,10,12,60,18,hDlg,NULL,g_hInstance,NULL);
            eMat=CreateWindow("EDIT","",WS_CHILD|WS_VISIBLE|WS_BORDER,80,10,180,20,hDlg,NULL,g_hInstance,NULL);
            CreateWindow("STATIC","Largura:",WS_CHILD|WS_VISIBLE,10,40,60,18,hDlg,NULL,g_hInstance,NULL);
            eW=CreateWindow("EDIT","",WS_CHILD|WS_VISIBLE|WS_BORDER|ES_NUMBER,80,38,60,20,hDlg,NULL,g_hInstance,NULL);
            CreateWindow("STATIC","Altura:",WS_CHILD|WS_VISIBLE,150,40,50,18,hDlg,NULL,g_hInstance,NULL);
            eH=CreateWindow("EDIT","",WS_CHILD|WS_VISIBLE|WS_BORDER|ES_NUMBER,205,38,55,20,hDlg,NULL,g_hInstance,NULL);
            CreateWindow("STATIC","Espessura:",WS_CHILD|WS_VISIBLE,10,68,65,18,hDlg,NULL,g_hInstance,NULL);
            eT=CreateWindow("EDIT","",WS_CHILD|WS_VISIBLE|WS_BORDER|ES_NUMBER,80,66,60,20,hDlg,NULL,g_hInstance,NULL);
            CreateWindow("STATIC","Notas:",WS_CHILD|WS_VISIBLE,10,96,50,18,hDlg,NULL,g_hInstance,NULL);
            eNotes=CreateWindow("EDIT","",WS_CHILD|WS_VISIBLE|WS_BORDER|ES_AUTOHSCROLL,80,94,180,20,hDlg,NULL,g_hInstance,NULL);
            CreateWindow("BUTTON","Salvar",WS_CHILD|WS_VISIBLE|BS_DEFPUSHBUTTON,80,125,80,24,hDlg,(HMENU)IDOK,g_hInstance,NULL);
            CreateWindow("BUTTON","Cancelar",WS_CHILD|WS_VISIBLE|BS_PUSHBUTTON,180,125,80,24,hDlg,(HMENU)IDCANCEL,g_hInstance,NULL);
            return 0;
        case WM_COMMAND:
            if (LOWORD(wParam)==IDOK){ char mat[64],w[16],h[16],t[16],notes[256];
                GetWindowTextA(eMat,mat,sizeof(mat)); GetWindowTextA(eW,w,sizeof(w)); GetWindowTextA(eH,h,sizeof(h)); GetWindowTextA(eT,t,sizeof(t)); GetWindowTextA(eNotes,notes,sizeof(notes));
                if(!mat[0]||!w[0]||!h[0]||!t[0]){ MessageBox(hDlg,"Campos obrigatorios faltando","Add",MB_OK|MB_ICONWARNING); return 0; }
                char json[640]; if (notes[0]) wsprintfA(json,"{\"width_mm\":%d,\"height_mm\":%d,\"thickness_mm\":%d,\"material\":\"%s\",\"notes\":\"%s\"}",atoi(w),atoi(h),atoi(t),mat,notes);
                else wsprintfA(json,"{\"width_mm\":%d,\"height_mm\":%d,\"thickness_mm\":%d,\"material\":\"%s\",\"notes\":null}",atoi(w),atoi(h),atoi(t),mat);
                char* resp=NULL; if (HttpRequestEx("POST","/add",json,&resp)){ if(resp) free(resp); DestroyWindow(hDlg);} else { MessageBox(hDlg,"Falha ao adicionar","Erro",MB_OK|MB_ICONERROR);} return 0; }
            if (LOWORD(wParam)==IDCANCEL){ DestroyWindow(hDlg); return 0; }
            break;
        case WM_CLOSE: DestroyWindow(hDlg); return 0;
    }
    return DefWindowProc(hDlg,uMsg,wParam,lParam);
}

static void ShowAddDialog(HWND owner){
    HWND dlg = CreateWindowExA(WS_EX_DLGMODALFRAME, "STATIC", "Adicionar Resto", WS_POPUP|WS_CAPTION|WS_SYSMENU,
        CW_USEDEFAULT,CW_USEDEFAULT,280,190, owner, NULL, g_hInstance, NULL);
    if (!dlg) return; SetWindowLongPtr(dlg, GWLP_WNDPROC, (LONG_PTR)AddWndProc); ShowWindow(dlg, SW_SHOW); UpdateWindow(dlg);
    MSG m; BOOL running=TRUE; while(running && GetMessage(&m,NULL,0,0)){ if(!IsDialogMessage(dlg,&m)){ TranslateMessage(&m); DispatchMessage(&m);} if(!IsWindow(dlg)) running=FALSE; }
    RefreshListView();
}

LRESULT CALLBACK WindowProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam) {
    switch(uMsg){
        case WM_CREATE: {
            ParseProxyUrl(); RECT rc; GetClientRect(hwnd,&rc);
            g_hTab = CreateWindowEx(0, WC_TABCONTROL, "", WS_CHILD|WS_VISIBLE|WS_CLIPSIBLINGS, 0,0, rc.right, 44, hwnd, (HMENU)IDC_TAB, g_hInstance, NULL);
            TCITEMA ti; ZeroMemory(&ti,sizeof(ti)); ti.mask=TCIF_TEXT; ti.pszText="Retalhos"; TabCtrl_InsertItem(g_hTab,0,&ti); ti.pszText="Otimizar"; TabCtrl_InsertItem(g_hTab,1,&ti); ti.pszText="Estado"; TabCtrl_InsertItem(g_hTab,2,&ti);
            CreateRetalhosPage(hwnd); CreateOtimizarPage(hwnd); CreateEstadoPage(hwnd); SwitchTab(0);
            g_hStatusBar = CreateWindowEx(0, STATUSCLASSNAME, NULL, WS_CHILD|WS_VISIBLE|SBARS_SIZEGRIP, 0,0,0,0, hwnd, (HMENU)IDC_STATUSBAR, g_hInstance, NULL);
            RefreshListView();
            return 0; }
        case WM_COMMAND:
            switch(LOWORD(wParam)){
                case IDC_BTN_ADD: ShowAddDialog(hwnd); return 0;
                case IDC_BTN_REMOVE: RemoveSelected(); return 0;
                case IDC_BTN_FIND: DoSearch(); return 0;
                case IDC_BTN_REFRESH: RefreshListView(); return 0;
                case IDC_BTN_STATUS: LoadStatus(); return 0;
            }
            break;
        case WM_NOTIFY:
            if (wParam==IDC_TAB && ((LPNMHDR)lParam)->code==TCN_SELCHANGE){ int idx=TabCtrl_GetCurSel(g_hTab); SwitchTab(idx); return 0; }
            break;
        case WM_SIZE:
            SendMessage(g_hStatusBar, WM_SIZE, 0, 0);
            if (g_hTab) MoveWindow(g_hTab, 0, 0, LOWORD(lParam), 44, TRUE);
            if (g_hListView) SetWindowPos(g_hListView,NULL,10,50, LOWORD(lParam)-20, HIWORD(lParam)-90, SWP_NOZORDER);
            if (g_hListSearch) SetWindowPos(g_hListSearch,NULL,10,45, LOWORD(lParam)-20, HIWORD(lParam)-90, SWP_NOZORDER);
            return 0;
        case WM_DESTROY: PostQuitMessage(0); return 0;
    }
    return DefWindowProc(hwnd,uMsg,wParam,lParam);
}

/* JSON parser (unchanged from original) */
void ParseRestoList(const char* json) {
    const char* p = json; g_inventoryCount = 0;
    while (*p && g_inventoryCount < MAX_RESTOS) {
        p = strstr(p, "{\"id\":"); if (!p) break;
        Resto* r = &g_inventory[g_inventoryCount]; ZeroMemory(r, sizeof(Resto));
        if (sscanf(p, "{\"id\":%d", &r->id) == 1) {
            const char* w = strstr(p, "\"width_mm\":"); if (w) sscanf(w, "\"width_mm\":%d", &r->width_mm);
            const char* h = strstr(p, "\"height_mm\":"); if (h) sscanf(h, "\"height_mm\":%d", &r->height_mm);
            const char* t = strstr(p, "\"thickness_mm\":"); if (t) sscanf(t, "\"thickness_mm\":%d", &r->thickness_mm);
            const char* m = strstr(p, "\"material\":\""); if (m){ m+=12; const char* e=strchr(m,'"'); if(e){ int len=(int)(e-m); if(len>= (int)sizeof(r->material)) len=sizeof(r->material)-1; memcpy(r->material,m,len); r->material[len]='\0'; } }
            const char* n = strstr(p, "\"notes\":"); if (n){ n+=8; if (strncmp(n,"null",4)!=0 && *n=='\"'){ n++; const char* e=strchr(n,'"'); if(e){ int len=(int)(e-n); if(len>=(int)sizeof(r->notes)) len=sizeof(r->notes)-1; memcpy(r->notes,n,len); r->notes[len]='\0'; } } }
            g_inventoryCount++;
        }
        p++;
    }
}

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE hPrev, LPSTR lpCmdLine, int nCmdShow){
    g_hInstance = hInstance; INITCOMMONCONTROLSEX icc; icc.dwSize=sizeof(icc); icc.dwICC=ICC_LISTVIEW_CLASSES|ICC_BAR_CLASSES|ICC_TAB_CLASSES; InitCommonControlsEx(&icc);
    WNDCLASSEXA wc; ZeroMemory(&wc,sizeof(wc)); wc.cbSize=sizeof(wc); wc.lpfnWndProc=WindowProc; wc.hInstance=hInstance; wc.hCursor=LoadCursor(NULL,IDC_ARROW); wc.hbrBackground=(HBRUSH)(COLOR_3DFACE+1); wc.lpszClassName="RetListerWindowClass"; RegisterClassExA(&wc);
    g_hMainWindow = CreateWindowExA(0, "RetListerWindowClass", "RetLister - Gestao de Retalhos", WS_OVERLAPPEDWINDOW, CW_USEDEFAULT,CW_USEDEFAULT, WINDOW_WIDTH, WINDOW_HEIGHT, NULL, NULL, hInstance, NULL);
    if (!g_hMainWindow){ MessageBox(NULL,"Failed to create window","Error",MB_OK|MB_ICONERROR); return 1; }
    ShowWindow(g_hMainWindow,nCmdShow); UpdateWindow(g_hMainWindow);
    MSG msg; while(GetMessage(&msg,NULL,0,0)){ TranslateMessage(&msg); DispatchMessage(&msg);} return (int)msg.wParam;
}
    switch (uMsg) {
        case WM_CREATE: {
            ParseProxyUrl();
            RECT rc; GetClientRect(hwnd, &rc);

            g_hTab = CreateWindowEx(0, WC_TABCONTROL, "", WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0, 0, rc.right, 44, hwnd, (HMENU)IDC_TAB, g_hInstance, NULL);

            TCITEMA ti; ZeroMemory(&ti, sizeof(ti)); ti.mask = TCIF_TEXT;
            ti.pszText = "Retalhos"; TabCtrl_InsertItem(g_hTab, 0, &ti);
            ti.pszText = "Otimizar"; TabCtrl_InsertItem(g_hTab, 1, &ti);
            ti.pszText = "Estado";   TabCtrl_InsertItem(g_hTab, 2, &ti);

            /* Create controls for all pages; toggle visibility */
            CreateRetalhosPage(hwnd);
            CreateOtimizarPage(hwnd);
            CreateEstadoPage(hwnd);
            SwitchTab(0);

            g_hStatusBar = CreateWindowEx(0, STATUSCLASSNAME, NULL,
                WS_CHILD | WS_VISIBLE | SBARS_SIZEGRIP,
                0, 0, 0, 0, hwnd, (HMENU)IDC_STATUSBAR, g_hInstance, NULL);

            RefreshListView();
            break; }
            
        case WM_COMMAND:
            switch (LOWORD(wParam)) {
                case IDC_BTN_ADD: {
                    /* Simple inline ADD via input boxes */
                    char mat[64] = "", notes[256] = "";
                    char wbuf[16] = "", hbuf[16] = "", tbuf[16] = "";
                    if (DialogBoxParamA(NULL, NULL, hwnd, NULL, 0) || 1) {
                        /* For now, prompt via InputBox-like sequence */
                        if (IDOK != MessageBox(hwnd, "Adicionar item com valores predefinidos? (edicao completa em breve)", "Adicionar", MB_OKCANCEL | MB_ICONQUESTION)) break;
                        lstrcpyA(mat, "Aco"); lstrcpyA(notes, ""); lstrcpyA(wbuf, "1000"); lstrcpyA(hbuf, "500"); lstrcpyA(tbuf, "3");
                        char json[512];
                        wsprintfA(json, "{\"width_mm\":%d,\"height_mm\":%d,\"thickness_mm\":%d,\"material\":\"%s\",\"notes\":%s}",
                            atoi(wbuf), atoi(hbuf), atoi(tbuf), mat, "null");
                        char* resp = NULL;
                        if (HttpRequestEx("POST", "/add", json, &resp)) { free(resp); RefreshListView(); }
                        else MessageBox(hwnd, "Falha ao adicionar", "Erro", MB_OK | MB_ICONERROR);
                    }
                    break; }
                case IDC_BTN_REMOVE:
                    RemoveSelected();
                    break;
                case WM_COMMAND:
                    switch (LOWORD(wParam)) {
                        case IDC_BTN_ADD:
                            ShowAddDialog(hwnd);
                            break;
                        case IDC_BTN_REMOVE:
                            RemoveSelected();
                            break;
                        case IDC_BTN_FIND:
                            DoSearch();
                            break;
                        case IDC_BTN_REFRESH:
                            RefreshListView();
                            break;
                        case IDC_BTN_STATUS:
                            LoadStatus();
                            break;

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, LPSTR lpCmdLine, int nCmdShow) {
    WNDCLASSEXA wc;
    MSG msg;
    INITCOMMONCONTROLSEX icex;
    
    g_hInstance = hInstance;
    
    icex.dwSize = sizeof(INITCOMMONCONTROLSEX);
    icex.dwICC = ICC_LISTVIEW_CLASSES | ICC_BAR_CLASSES;
    InitCommonControlsEx(&icex);
    
    memset(&wc, 0, sizeof(wc));
    wc.cbSize = sizeof(WNDCLASSEXA);
    wc.lpfnWndProc = WindowProc;
    wc.hInstance = hInstance;
    wc.hCursor = LoadCursor(NULL, IDC_ARROW);
    wc.hbrBackground = (HBRUSH)(COLOR_3DFACE + 1);
    wc.lpszClassName = "RetListerWindowClass";
    
    RegisterClassExA(&wc);

    /* Register Add dialog window class */
    WNDCLASSEXA wca; ZeroMemory(&wca, sizeof(wca));
    wca.cbSize = sizeof(wca);
    wca.lpfnWndProc = DefWindowProcA; /* will subclass later */
    wca.hInstance = hInstance;
    wca.hCursor = LoadCursor(NULL, IDC_ARROW);
    wca.hbrBackground = (HBRUSH)(COLOR_3DFACE+1);
    wca.lpszClassName = "AddRestoWnd";
    RegisterClassExA(&wca);
    
    g_hMainWindow = CreateWindowExA(
        0,
        "RetListerWindowClass",
        "RetLister - Gestao de Retalhos",
        WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT, CW_USEDEFAULT,
        WINDOW_WIDTH, WINDOW_HEIGHT,
        NULL, NULL, hInstance, NULL
    );
    
    if (!g_hMainWindow) {
        MessageBox(NULL, "Failed to create window", "Error", MB_OK | MB_ICONERROR);
        return 1;
    }
    
    ShowWindow(g_hMainWindow, nCmdShow);
    UpdateWindow(g_hMainWindow);
    
    while (GetMessage(&msg, NULL, 0, 0)) {
        TranslateMessage(&msg);
        DispatchMessage(&msg);
    }
    
    return (int)msg.wParam;
}

/* ---------------- Add Dialog (custom window) ---------------- */
static LRESULT CALLBACK AddWndProc(HWND hDlg, UINT uMsg, WPARAM wParam, LPARAM lParam) {
    static HWND eMat, eW, eH, eT, eNotes;
    switch (uMsg) {
        case WM_CREATE: {
            CreateWindow("STATIC", "Material:", WS_CHILD | WS_VISIBLE, 10, 12, 60, 18, hDlg, NULL, g_hInstance, NULL);
            eMat = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER, 80, 10, 180, 20, hDlg, NULL, g_hInstance, NULL);
            CreateWindow("STATIC", "Largura:", WS_CHILD | WS_VISIBLE, 10, 40, 60, 18, hDlg, NULL, g_hInstance, NULL);
            eW = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, 80, 38, 60, 20, hDlg, NULL, g_hInstance, NULL);
            CreateWindow("STATIC", "Altura:", WS_CHILD | WS_VISIBLE, 150, 40, 50, 18, hDlg, NULL, g_hInstance, NULL);
            eH = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, 205, 38, 55, 20, hDlg, NULL, g_hInstance, NULL);
            CreateWindow("STATIC", "Espessura:", WS_CHILD | WS_VISIBLE, 10, 68, 65, 18, hDlg, NULL, g_hInstance, NULL);
            eT = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, 80, 66, 60, 20, hDlg, NULL, g_hInstance, NULL);
            CreateWindow("STATIC", "Notas:", WS_CHILD | WS_VISIBLE, 10, 96, 50, 18, hDlg, NULL, g_hInstance, NULL);
            eNotes = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL, 80, 94, 180, 20, hDlg, NULL, g_hInstance, NULL);
            CreateWindow("BUTTON", "Salvar", WS_CHILD | WS_VISIBLE | BS_DEFPUSHBUTTON, 80, 125, 80, 24, hDlg, (HMENU)IDOK, g_hInstance, NULL);
            CreateWindow("BUTTON", "Cancelar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON, 180, 125, 80, 24, hDlg, (HMENU)IDCANCEL, g_hInstance, NULL);
            break; }
        case WM_COMMAND:
            switch (LOWORD(wParam)) {
                case IDOK: {
                    char mat[64], w[16], h[16], t[16], notes[256];
                    GetWindowTextA(eMat, mat, sizeof(mat));
                    GetWindowTextA(eW, w, sizeof(w));
                    GetWindowTextA(eH, h, sizeof(h));
                    GetWindowTextA(eT, t, sizeof(t));
                    GetWindowTextA(eNotes, notes, sizeof(notes));
                    if (!mat[0] || !w[0] || !h[0] || !t[0]) { MessageBox(hDlg, "Campos obrigatorios faltando", "Add", MB_OK | MB_ICONWARNING); break; }
                    char json[640];
                    if (notes[0])
                        wsprintfA(json, "{\"width_mm\":%d,\"height_mm\":%d,\"thickness_mm\":%d,\"material\":\"%s\",\"notes\":\"%s\"}", atoi(w), atoi(h), atoi(t), mat, notes);
                    else
                        wsprintfA(json, "{\"width_mm\":%d,\"height_mm\":%d,\"thickness_mm\":%d,\"material\":\"%s\",\"notes\":null}", atoi(w), atoi(h), atoi(t), mat);
                    char* resp = NULL;
                    if (HttpRequestEx("POST", "/add", json, &resp)) { if (resp) free(resp); EndDialog(hDlg, IDOK); }
                    else { MessageBox(hDlg, "Falha ao adicionar", "Erro", MB_OK | MB_ICONERROR); }
                    break; }
                case IDCANCEL:
                    EndDialog(hDlg, IDCANCEL);
                    break;
            }
            break;
        case WM_CLOSE:
            EndDialog(hDlg, IDCANCEL);
            break;
    }
    return 0;
}

static void ShowAddDialog(HWND owner) {
    /* Create a simple modal window */
    HWND dlg = CreateWindowExA(WS_EX_DLGMODALFRAME, "AddRestoWnd", "Adicionar Resto",
        WS_POPUP | WS_CAPTION | WS_SYSMENU, CW_USEDEFAULT, CW_USEDEFAULT, 280, 190,
        owner, NULL, g_hInstance, NULL);
    if (!dlg) return;
    /* Subclass to custom proc */
    SetWindowLongPtr(dlg, GWLP_WNDPROC, (LONG_PTR)AddWndProc);
    SetWindowPos(dlg, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE);
    ShowWindow(dlg, SW_SHOW);
    UpdateWindow(dlg);
    /* Local modal loop */
    MSG m;
    BOOL running = TRUE;
    while (running && GetMessage(&m, NULL, 0, 0)) {
        if (!IsDialogMessage(dlg, &m)) {
            TranslateMessage(&m);
            DispatchMessage(&m);
        }
        if (!IsWindow(dlg)) running = FALSE; /* closed */
    }
    /* after close, refresh list */
    RefreshListView();
}
