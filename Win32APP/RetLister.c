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
#define WINDOW_WIDTH 1400
#define WINDOW_HEIGHT 900
#define MAX_RESTOS 1000
/* Layout */
#define TAB_HEIGHT 44
#define PAGE_MARGIN 10
#define PAGE_TOP (TAB_HEIGHT + PAGE_MARGIN)
#define ROW_GAP 36

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
#define IDC_ED_PROXY_URL 1207
#define IDC_BTN_SAVE_PROXY 1208
#define IDC_SLIDER_FONTSIZE 1209
#define IDC_CHK_AUTOREFRESH 1210

/* Corte page */
#define IDC_ED_CUT_W      1301
#define IDC_ED_CUT_H      1302
#define IDC_ED_CUT_T      1303
#define IDC_ED_CUT_MAT    1304
#define IDC_ED_CUT_QTY    1305
#define IDC_BTN_ADD_CUT   1306
#define IDC_LIST_CUTS     1307
#define IDC_BTN_OPTIMIZE  1308
#define IDC_ED_RESULT     1309
#define IDC_BTN_RESET_CUTS 1310
#define IDC_BTN_CONFIRM_CUTS 1311

#define MAX_JSON_BUFFER 262144

#define TIMER_AUTOREFRESH 1
#define AUTOREFRESH_INTERVAL 30000

/* Globals */
HINSTANCE g_hInstance;
HWND g_hMainWindow, g_hTab, g_hStatusBar;
HWND g_hListView; /* Retalhos */
HWND g_hEdW, g_hEdH, g_hEdT, g_hEdMat, g_hBtnFind, g_hListSearch; /* Otimizar */
HWND g_hLblWCap, g_hLblHCap, g_hLblTCap, g_hLblMatCap; /* Otimizar captions */
HWND g_hLblProxy, g_hLblMain, g_hLblUptime, g_hLblDb, g_hLblPending, g_hBtnStatus; /* Estado dynamic */
HWND g_hCapProxy, g_hCapMain, g_hCapUptime, g_hCapDb, g_hCapPending; /* Estado captions */
HWND g_hEdProxyUrl, g_hBtnSaveProxy, g_hCapProxyUrl; /* Proxy config */
HWND g_hSliderFont, g_hCapFontSize; /* Font size */
HWND g_hChkAutoRefresh; /* Auto-refresh */
HWND g_hEdCutW, g_hEdCutH, g_hEdCutT, g_hEdCutMat, g_hEdCutQty; /* Corte inputs */
HWND g_hListCuts, g_hCanvasResult, g_hBtnAddCut, g_hBtnOptimize, g_hBtnResetCuts, g_hBtnConfirmCuts; /* Corte controls */
HWND g_hLblCutWCap, g_hLblCutHCap, g_hLblCutTCap, g_hLblCutMatCap, g_hLblCutQtyCap; /* Corte captions */
int g_canvasScrollPos = 0;
int g_canvasTotalHeight = 0;

char g_optimizeResult[MAX_JSON_BUFFER] = {0};
char g_ProxyHost[128] = {0};
INTERNET_PORT g_ProxyPort = 80;
HFONT g_hAppFont = NULL;
HFONT g_hListViewFont = NULL;
int g_fontSize = 14;
BOOL g_isOffline = FALSE;
BOOL g_autoRefresh = FALSE;

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

int GetJsonInt(const char* json, const char* key) {
    if (!json || !key) return 0;
    
    const char* pos = strstr(json, key);
    if (!pos) return 0;
    
    pos += strlen(key);
    
    while (*pos && (*pos < '0' || *pos > '9') && *pos != '-') {
        pos++;
    }
    
    return atoi(pos);
}

float GetJsonFloat(const char* json, const char* key) {
    if (!json || !key) return 0.0f;
    
    const char* pos = strstr(json, key);
    if (!pos) return 0.0f;
    
    pos += strlen(key);
    
    while (*pos && (*pos < '0' || *pos > '9') && *pos != '-' && *pos != '.') {
        pos++;
    }
    
    return (float)atof(pos);
}

const char* GetJsonObjectEnd(const char* start) {
    if (!start || *start != '{') return NULL;
    
    int braceCount = 0;
    const char* p = start;
    
    while (*p) {
        if (*p == '{') {
            braceCount++;
        } else if (*p == '}') {
            braceCount--;
            if (braceCount == 0) {
                return p; 
            }
        }
        p++;
    }
    return NULL; 
}

static void ParseProxyUrlFromString(const char* url) {
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

static void ParseProxyUrl(void) {
    ParseProxyUrlFromString(PROXY_URL);
}

static void CreateAppFont(void) {
    if (g_hAppFont) DeleteObject(g_hAppFont);
    g_hAppFont = CreateFont(g_fontSize, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, 
                            DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, 
                            DEFAULT_QUALITY, DEFAULT_PITCH | FF_DONTCARE, "Tahoma");
    
    if (g_hListViewFont) DeleteObject(g_hListViewFont);
    int listViewFontSize = g_fontSize * 2;
    g_hListViewFont = CreateFont(listViewFontSize, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, 
                                 DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, 
                                 DEFAULT_QUALITY, DEFAULT_PITCH | FF_DONTCARE, "Tahoma");
}

static void ApplyFontToControl(HWND hwnd) {
    if (hwnd && g_hAppFont) SendMessage(hwnd, WM_SETFONT, (WPARAM)g_hAppFont, TRUE);
}

static void SetListViewRowHeight(HWND hListView, int rowHeight) {
    HIMAGELIST hImageList = ImageList_Create(1, rowHeight, ILC_COLOR, 1, 0);
    if (hImageList) {
        ListView_SetImageList(hListView, hImageList, LVSIL_SMALL);
    }
}

static void ApplyFontToAllControls(void) {
    if (g_hListView && g_hListViewFont) {
        SendMessage(g_hListView, WM_SETFONT, (WPARAM)g_hListViewFont, TRUE);
    }
    if (g_hListSearch && g_hListViewFont) {
        SendMessage(g_hListSearch, WM_SETFONT, (WPARAM)g_hListViewFont, TRUE);
    }
    
    int rowHeight = (g_fontSize * 2) + 16;
    SetListViewRowHeight(g_hListView, rowHeight);
    SetListViewRowHeight(g_hListSearch, rowHeight);
    
    InvalidateRect(g_hMainWindow, NULL, TRUE);
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

void ParseRestoList(const char* json) {
    const char* p = json; g_inventoryCount = 0;
    while (*p && g_inventoryCount < MAX_RESTOS) {
        p = strstr(p, "{\"id\":"); if (!p) break;
        
        // Find the end of this record by counting braces
        int braceCount = 0;
        const char* recordStart = p;
        const char* scan = p;
        while (*scan) {
            if (*scan == '{') braceCount++;
            else if (*scan == '}') {
                braceCount--;
                if (braceCount == 0) break;
            }
            scan++;
        }
        const char* recordEnd = scan + 1;
        
        // Copy this record to a temp buffer for safe parsing
        int recordLen = (int)(recordEnd - recordStart);
        if (recordLen > 0 && recordLen < 2048) {
            char record[2048];
            memcpy(record, recordStart, recordLen);
            record[recordLen] = '\0';
            
            Resto* r = &g_inventory[g_inventoryCount]; ZeroMemory(r, sizeof(Resto));
            if (sscanf(record, "{\"id\":%d", &r->id) == 1) {
                const char* w = strstr(record, "\"width_mm\":"); 
                if (w) sscanf(w, "\"width_mm\":%d", &r->width_mm);
                const char* h = strstr(record, "\"height_mm\":"); 
                if (h) sscanf(h, "\"height_mm\":%d", &r->height_mm);
                const char* t = strstr(record, "\"thickness_mm\":"); 
                if (t) sscanf(t, "\"thickness_mm\":%d", &r->thickness_mm);
                const char* m = strstr(record, "\"material\":\""); 
                if (m){ m+=12; const char* e=strchr(m,'"'); if(e){ int len=(int)(e-m); if(len>= (int)sizeof(r->material)) len=sizeof(r->material)-1; memcpy(r->material,m,len); r->material[len]='\0'; } }
                const char* n = strstr(record, "\"notes\":"); 
                if (n){ n+=8; if (strncmp(n,"null",4)!=0 && *n=='\"'){ n++; const char* e=strchr(n,'"'); if(e){ int len=(int)(e-n); if(len>=(int)sizeof(r->notes)) len=sizeof(r->notes)-1; memcpy(r->notes,n,len); r->notes[len]='\0'; } } }
                g_inventoryCount++;
            }
        }
        p = recordEnd;
    }
}

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
    char* response=NULL; 
    if (HttpRequestEx("GET","/list",NULL,&response)) { 
        ParseRestoList(response); 
        free(response);
        g_isOffline = FALSE;
    } else {
        g_isOffline = TRUE;
    }
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
    if (g_isOffline) {
        wsprintfA(status, "%d retalhos [OFFLINE]", g_inventoryCount);
    } else {
        wsprintfA(status, "%d retalhos", g_inventoryCount);
    }
    SendMessage(g_hStatusBar, SB_SETTEXT, 0, (LPARAM)status);
}

void RemoveSelected(void) {
    int sel = ListView_GetNextItem(g_hListView, -1, LVNI_SELECTED);
    if (sel<0 || sel>=g_inventoryCount){ MessageBox(g_hMainWindow, "Sem selecao", "Remover", MB_OK|MB_ICONWARNING); return; }
    int id = g_inventory[sel].id; char msg[64]; wsprintfA(msg, "Remover resto #%d?", id);
    if (IDYES != MessageBox(g_hMainWindow, msg, "Confirmar", MB_YESNO|MB_ICONQUESTION)) return;
    char path[64]; wsprintfA(path, "/remove/%d", id); if (!HttpRequestEx("DELETE", path, NULL, NULL)) { MessageBox(g_hMainWindow, "Erro ao remover", "Erro", MB_OK|MB_ICONERROR); return; }
    RefreshListView();
}

static void CreateRetalhosPage(HWND hwndParent) {
    CreateWindow("BUTTON", "Adicionar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        PAGE_MARGIN, PAGE_TOP, 100, 26, hwndParent, (HMENU)IDC_BTN_ADD, g_hInstance, NULL);
    CreateWindow("BUTTON", "Remover", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        PAGE_MARGIN + 110, PAGE_TOP, 100, 26, hwndParent, (HMENU)IDC_BTN_REMOVE, g_hInstance, NULL);
    CreateWindow("BUTTON", "Atualizar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        PAGE_MARGIN + 220, PAGE_TOP, 100, 26, hwndParent, (HMENU)IDC_BTN_REFRESH, g_hInstance, NULL);
    CreateListView(hwndParent);
}

static void CreateOtimizarPage(HWND hwndParent) {
    g_hLblWCap = CreateWindow("STATIC", "Largura (mm):", WS_CHILD | WS_VISIBLE, PAGE_MARGIN, PAGE_TOP + 2, 90, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdW = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, PAGE_MARGIN + 95, PAGE_TOP, 80, 22, hwndParent, (HMENU)IDC_ED_W, g_hInstance, NULL);
    g_hLblHCap = CreateWindow("STATIC", "Altura (mm):", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 185, PAGE_TOP + 2, 80, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdH = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, PAGE_MARGIN + 270, PAGE_TOP, 80, 22, hwndParent, (HMENU)IDC_ED_H, g_hInstance, NULL);
    g_hLblTCap = CreateWindow("STATIC", "Espessura:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 360, PAGE_TOP + 2, 70, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdT = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, PAGE_MARGIN + 435, PAGE_TOP, 60, 22, hwndParent, (HMENU)IDC_ED_T, g_hInstance, NULL);
    g_hLblMatCap = CreateWindow("STATIC", "Material:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 505, PAGE_TOP + 2, 60, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdMat = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER, PAGE_MARGIN + 570, PAGE_TOP, 120, 22, hwndParent, (HMENU)IDC_ED_MAT, g_hInstance, NULL);
    g_hBtnFind = CreateWindow("BUTTON", "Procurar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON, PAGE_MARGIN + 695, PAGE_TOP - 1, 90, 24, hwndParent, (HMENU)IDC_BTN_FIND, g_hInstance, NULL);
    g_hListSearch = CreateWindowEx(WS_EX_CLIENTEDGE, WC_LISTVIEW, "", WS_CHILD | WS_VISIBLE | LVS_REPORT | WS_BORDER,
        PAGE_MARGIN, PAGE_TOP + ROW_GAP, WINDOW_WIDTH - 30, WINDOW_HEIGHT - 120, hwndParent, (HMENU)IDC_LISTSEARCH, g_hInstance, NULL);
    ListView_SetExtendedListViewStyle(g_hListSearch, LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES);
    LVCOLUMNA lvc; ZeroMemory(&lvc, sizeof(lvc)); lvc.mask = LVCF_TEXT | LVCF_WIDTH;
    lvc.pszText = "ID"; lvc.cx = 60; ListView_InsertColumn(g_hListSearch, 0, &lvc);
    lvc.pszText = "Largura"; lvc.cx = 100; ListView_InsertColumn(g_hListSearch, 1, &lvc);
    lvc.pszText = "Altura"; lvc.cx = 100; ListView_InsertColumn(g_hListSearch, 2, &lvc);
    lvc.pszText = "Espessura"; lvc.cx = 100; ListView_InsertColumn(g_hListSearch, 3, &lvc);
    lvc.pszText = "Material"; lvc.cx = 120; ListView_InsertColumn(g_hListSearch, 4, &lvc);
    lvc.pszText = "Notas"; lvc.cx = 400; ListView_InsertColumn(g_hListSearch, 5, &lvc);
}

static void LoadProxyUrl(void) {
    char path[MAX_PATH];
    GetModuleFileNameA(NULL, path, sizeof(path));
    char* slash = strrchr(path, '\\');
    if (slash) { strcpy(slash + 1, "proxy.cfg"); }
    FILE* f = fopen(path, "r");
    if (f) {
        char url[256];
        if (fgets(url, sizeof(url), f)) {
            url[strcspn(url, "\r\n")] = 0;
            if (strlen(url) > 0) {
                SetWindowTextA(g_hEdProxyUrl, url);
                ParseProxyUrlFromString(url);
            }
        }
        fclose(f);
    } else {
        SetWindowTextA(g_hEdProxyUrl, PROXY_URL);
        ParseProxyUrl();
    }
}

static void SaveProxyUrl(void) {
    char url[256];
    GetWindowTextA(g_hEdProxyUrl, url, sizeof(url));
    char path[MAX_PATH];
    GetModuleFileNameA(NULL, path, sizeof(path));
    char* slash = strrchr(path, '\\');
    if (slash) { strcpy(slash + 1, "proxy.cfg"); }
    FILE* f = fopen(path, "w");
    if (f) {
        fprintf(f, "%s", url);
        fclose(f);
        ParseProxyUrlFromString(url);
        MessageBox(g_hMainWindow, "Proxy URL guardado!", "Info", MB_OK | MB_ICONINFORMATION);
    }
}

static void SaveUISettings(void) {
    char path[MAX_PATH];
    GetModuleFileNameA(NULL, path, sizeof(path));
    char* slash = strrchr(path, '\\');
    if (slash) { strcpy(slash + 1, "ui.cfg"); }
    FILE* f = fopen(path, "w");
    if (f) {
        fprintf(f, "fontSize=%d\n", g_fontSize);
        for (int i = 0; i < 6; i++) {
            int width = ListView_GetColumnWidth(g_hListView, i);
            fprintf(f, "col%d=%d\n", i, width);
        }
        for (int i = 0; i < 6; i++) {
            int width = ListView_GetColumnWidth(g_hListSearch, i);
            fprintf(f, "search_col%d=%d\n", i, width);
        }
        fclose(f);
    }
}

static void LoadUISettings(void) {
    char path[MAX_PATH];
    GetModuleFileNameA(NULL, path, sizeof(path));
    char* slash = strrchr(path, '\\');
    if (slash) { strcpy(slash + 1, "ui.cfg"); }
    FILE* f = fopen(path, "r");
    if (f) {
        char line[256];
        int colWidths[6] = {60, 100, 100, 100, 120, 400};
        int searchColWidths[6] = {60, 100, 100, 100, 120, 400};
        
        while (fgets(line, sizeof(line), f)) {
            line[strcspn(line, "\r\n")] = 0;
            if (strncmp(line, "fontSize=", 9) == 0) {
                g_fontSize = atoi(line + 9);
                if (g_fontSize < 8) g_fontSize = 8;
                if (g_fontSize > 24) g_fontSize = 24;
            } else if (strncmp(line, "col", 3) == 0 && line[4] == '=') {
                int idx = line[3] - '0';
                if (idx >= 0 && idx < 6) {
                    colWidths[idx] = atoi(line + 5);
                }
            } else if (strncmp(line, "search_col", 10) == 0 && line[11] == '=') {
                int idx = line[10] - '0';
                if (idx >= 0 && idx < 6) {
                    searchColWidths[idx] = atoi(line + 12);
                }
            }
        }
        fclose(f);
        
        for (int i = 0; i < 6; i++) {
            ListView_SetColumnWidth(g_hListView, i, colWidths[i]);
            ListView_SetColumnWidth(g_hListSearch, i, searchColWidths[i]);
        }
        
        if (g_hSliderFont) {
            SendMessage(g_hSliderFont, TBM_SETPOS, TRUE, g_fontSize);
        }
    }
}

static void ConfirmCuttingPlan(void) {
    if (!g_optimizeResult[0]) {
        MessageBox(g_hMainWindow, "Execute a otimizacao primeiro", "Confirmar", MB_OK | MB_ICONWARNING);
        return;
    }
    
    int result = MessageBox(g_hMainWindow, 
        "Confirmar o plano de corte?\n\nIsso vai remover as pranchas usadas do inventario.\n\nNOTA: Lembre-se de adicionar as sobras manualmente apos o corte.",
        "Confirmar Corte", MB_YESNO | MB_ICONQUESTION);
    
    if (result != IDYES) return;
    
    const char* p = strstr(g_optimizeResult, "\"used_planks\":[");
    if (!p) return;
    
    p += 15;
    
    // Buffer to hold the JSON list of IDs: {"ids":[1,2,3]}
    char jsonBatch[4096];
    strcpy(jsonBatch, "{\"ids\":[");
    int count = 0;
    
    int safeGuard = 0;
    while (*p && *p != ']' && safeGuard < 500) {
        safeGuard++;
        const char* plankStart = strchr(p, '{');
        if (!plankStart) break;
        
        const char* plankEnd = GetJsonObjectEnd(plankStart);
        if (!plankEnd) break;
        
        // Robust ID extraction
        int restoId = 0;
        const char* keyPos = strstr(plankStart, "\"resto_id\"");
        if (keyPos && keyPos < plankEnd) {
            restoId = GetJsonInt(plankStart, "\"resto_id\"");
        }
        
        if (restoId > 0) {
            char numBuf[32];
            wsprintfA(numBuf, "%s%d", count > 0 ? "," : "", restoId);
            strcat(jsonBatch, numBuf);
            count++;
        }
        
        p = plankEnd + 1;
    }
    
    strcat(jsonBatch, "]}");
    
    if (count > 0) {
        char* resp = NULL;
        if (HttpRequestEx("POST", "/delete_batch", jsonBatch, &resp)) {
            char msg[256];
            wsprintfA(msg, "Sucesso!\n\n%d pranchas foram removidas.", count);
            MessageBox(g_hMainWindow, msg, "Corte Confirmado", MB_OK | MB_ICONINFORMATION);
            if (resp) free(resp);
        } else {
            MessageBox(g_hMainWindow, "Erro ao conectar com servidor para deletar.", "Erro", MB_OK | MB_ICONERROR);
        }
    } else {
        MessageBox(g_hMainWindow, "Nenhuma prancha para remover.", "Info", MB_OK);
    }
    
    ListView_DeleteAllItems(g_hListCuts);
    g_optimizeResult[0] = '\0';
    InvalidateRect(g_hCanvasResult, NULL, TRUE);
    RefreshListView();
}

static void DoCutOptimization(void) {
    int cutCount = ListView_GetItemCount(g_hListCuts);
    if (cutCount == 0) {
        MessageBox(g_hMainWindow, "Adicione pecas primeiro", "Otimizar", MB_OK | MB_ICONWARNING);
        return;
    }
    
    char json[4096] = {0};
    strcpy(json, "{\"cuts\":[");
    
    for (int i = 0; i < cutCount; i++) {
        char width[16], height[16], thick[16], mat[64], qty[16];
        ListView_GetItemText(g_hListCuts, i, 0, width, sizeof(width));
        ListView_GetItemText(g_hListCuts, i, 1, height, sizeof(height));
        ListView_GetItemText(g_hListCuts, i, 2, thick, sizeof(thick));
        ListView_GetItemText(g_hListCuts, i, 3, mat, sizeof(mat));
        ListView_GetItemText(g_hListCuts, i, 4, qty, sizeof(qty));
        
        char cutJson[256];
        wsprintfA(cutJson, "%s{\"width_mm\":%s,\"height_mm\":%s,\"thickness_mm\":%s,\"material\":\"%s\",\"quantity\":%s}",
                 i > 0 ? "," : "", width, height, thick, mat, qty);
        strcat(json, cutJson);
    }
    strcat(json, "]}");
    
    char* resp = NULL;
    if (!HttpRequestEx("POST", "/optimize_cuts", json, &resp)) {
        MessageBox(g_hMainWindow, "Erro ao otimizar - servidor nao responde", "Erro", MB_OK | MB_ICONERROR);
        g_optimizeResult[0] = '\0';
        return;
    }
    
    if (!resp || !resp[0]) {
        MessageBox(g_hMainWindow, "Resposta vazia do servidor", "Erro", MB_OK | MB_ICONERROR);
        g_optimizeResult[0] = '\0';
        return;
    }
    
    char result[16384] = {0};
    strcpy(result, "=== PLANO DE CORTE ===\r\n\r\n");
    
    int totalPlaced = 0, totalRequested = 0;
    const char* placed = strstr(resp, "\"total_cuts_placed\":");
    if (placed) {
        sscanf(placed + 20, "%d", &totalPlaced);
    }
    const char* requested = strstr(resp, "\"total_cuts_requested\":");
    if (requested) {
        sscanf(requested + 23, "%d", &totalRequested);
    }
    
    const char* p = strstr(resp, "\"used_planks\":[");
    if (p) {
        p += 15;
        int plankNum = 1;
        while (*p && *p != ']') {
            const char* plankStart = strstr(p, "{\"resto_id\":");
            if (!plankStart) break;
            
            int restoId = 0, width = 0, height = 0;
            sscanf(plankStart, "{\"resto_id\":%d", &restoId);
            const char* ww = strstr(plankStart, "\"width_mm\":");
            if (ww) sscanf(ww, "\"width_mm\":%d", &width);
            const char* hh = strstr(plankStart, "\"height_mm\":");
            if (hh) sscanf(hh, "\"height_mm\":%d", &height);
            
            char plankInfo[256];
            wsprintfA(plankInfo, "PRANCHA #%d (ID:%d, %dx%dmm)\r\n", plankNum++, restoId, width, height);
            strcat(result, plankInfo);
            
            const char* cuts = strstr(plankStart, "\"cuts\":[");
            if (cuts) {
                cuts += 8;
                int cutNum = 1;
                while (*cuts && *cuts != ']') {
                    const char* cutStart = strstr(cuts, "{\"original_index\":");
                    if (!cutStart || cutStart > strstr(cuts, "]}")) break;
                    
                    int cx = 0, cy = 0, cw = 0, ch = 0;
                    sscanf(cutStart, "{\"original_index\":%*d,\"x\":%d,\"y\":%d,\"width\":%d,\"height\":%d", &cx, &cy, &cw, &ch);
                    
                    char cutInfo[128];
                    wsprintfA(cutInfo, "  Peca #%d: %dx%dmm em pos (%d,%d)\r\n", cutNum++, cw, ch, cx, cy);
                    strcat(result, cutInfo);
                    
                    cuts = strchr(cutStart + 1, '{');
                    if (!cuts) break;
                }
            }
            
            strcat(result, "\r\n");
            p = strchr(plankStart + 1, '{');
            if (!p) break;
        }
    }
    
    const char* eff = strstr(resp, "\"efficiency_percent\":");
    if (eff) {
        float effVal = 0;
        sscanf(eff, "\"efficiency_percent\":%f", &effVal);
        char effStr[128];
        wsprintfA(effStr, "\r\nEficiencia total: %.1f%%\r\n", effVal);
        strcat(result, effStr);
        wsprintfA(effStr, "Pecas colocadas: %d de %d\r\n", totalPlaced, totalRequested);
        strcat(result, effStr);
    }
    
    const char* unplaced = strstr(resp, "\"unplaced_cuts\":[");
    if (unplaced && strstr(unplaced, "\"unplaced_cuts\":[]") != unplaced) {
        strcat(result, "\r\n=== PECAS NAO COLOCADAS ===\r\n");
        unplaced += 17;
        int unplacedNum = 1;
        while (*unplaced && *unplaced != ']') {
            const char* cutStart = strstr(unplaced, "\"width_mm\":");
            if (!cutStart) break;
            
            int w = 0, h = 0, t = 0;
            sscanf(cutStart, "\"width_mm\":%d", &w);
            const char* hh = strstr(cutStart, "\"height_mm\":");
            if (hh) sscanf(hh, "\"height_mm\":%d", &h);
            const char* tt = strstr(cutStart, "\"thickness_mm\":");
            if (tt) sscanf(tt, "\"thickness_mm\":%d", &t);
            
            char mat[64] = "";
            const char* mm = strstr(cutStart, "\"material\":\"");
            if (mm) {
                mm += 12;
                const char* end = strchr(mm, '"');
                if (end) {
                    int len = (int)(end - mm);
                    if (len > 63) len = 63;
                    memcpy(mat, mm, len);
                    mat[len] = '\0';
                }
            }
            
            char unplacedInfo[128];
            wsprintfA(unplacedInfo, "  %d. %dx%dx%dmm %s\r\n", unplacedNum++, w, h, t, mat);
            strcat(result, unplacedInfo);
            
            unplaced = strchr(cutStart + 1, '{');
            if (!unplaced) break;
        }
    }
    
    if (totalPlaced == 0 && totalRequested > 0) {
        strcat(result, "\r\nAVISO: Nenhuma peca foi colocada!\r\n");
        strcat(result, "Verifique se ha retalhos disponiveis no inventario.\r\n");
    }
    
    if (resp) {
        strncpy(g_optimizeResult, resp, sizeof(g_optimizeResult) - 1);
        free(resp);
    }
    InvalidateRect(g_hCanvasResult, NULL, TRUE);
}

static void AddCutToPlan(void) {
    char w[16], h[16], t[16], m[64], q[16];
    GetWindowTextA(g_hEdCutW, w, sizeof(w));
    GetWindowTextA(g_hEdCutH, h, sizeof(h));
    GetWindowTextA(g_hEdCutT, t, sizeof(t));
    GetWindowTextA(g_hEdCutMat, m, sizeof(m));
    GetWindowTextA(g_hEdCutQty, q, sizeof(q));
    
    if (!w[0] || !h[0] || !t[0] || !m[0] || !q[0]) {
        MessageBox(g_hMainWindow, "Preencha todos os campos", "Adicionar", MB_OK | MB_ICONWARNING);
        return;
    }
    
    LVITEMA lvi;
    ZeroMemory(&lvi, sizeof(lvi));
    lvi.mask = LVIF_TEXT;
    lvi.iItem = ListView_GetItemCount(g_hListCuts);
    lvi.pszText = w;
    int idx = ListView_InsertItem(g_hListCuts, &lvi);
    
    ListView_SetItemText(g_hListCuts, idx, 1, h);
    ListView_SetItemText(g_hListCuts, idx, 2, t);
    ListView_SetItemText(g_hListCuts, idx, 3, m);
    ListView_SetItemText(g_hListCuts, idx, 4, q);
    
    SetWindowTextA(g_hEdCutW, "");
    SetWindowTextA(g_hEdCutH, "");
    SetWindowTextA(g_hEdCutT, "");
    SetWindowTextA(g_hEdCutMat, "");
    SetWindowTextA(g_hEdCutQty, "1");
}

static void CreateCortePage(HWND hwndParent) {
    g_hLblCutWCap = CreateWindow("STATIC", "Largura (mm):", WS_CHILD | WS_VISIBLE, PAGE_MARGIN, PAGE_TOP + 2, 90, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdCutW = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, PAGE_MARGIN + 95, PAGE_TOP, 60, 22, hwndParent, (HMENU)IDC_ED_CUT_W, g_hInstance, NULL);
    
    g_hLblCutHCap = CreateWindow("STATIC", "Altura (mm):", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 165, PAGE_TOP + 2, 80, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdCutH = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, PAGE_MARGIN + 245, PAGE_TOP, 60, 22, hwndParent, (HMENU)IDC_ED_CUT_H, g_hInstance, NULL);
    
    g_hLblCutTCap = CreateWindow("STATIC", "Espessura:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 315, PAGE_TOP + 2, 70, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdCutT = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, PAGE_MARGIN + 385, PAGE_TOP, 50, 22, hwndParent, (HMENU)IDC_ED_CUT_T, g_hInstance, NULL);
    
    g_hLblCutMatCap = CreateWindow("STATIC", "Material:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 445, PAGE_TOP + 2, 60, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdCutMat = CreateWindow("EDIT", "", WS_CHILD | WS_VISIBLE | WS_BORDER, PAGE_MARGIN + 505, PAGE_TOP, 80, 22, hwndParent, (HMENU)IDC_ED_CUT_MAT, g_hInstance, NULL);
    
    g_hLblCutQtyCap = CreateWindow("STATIC", "Qtd:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 595, PAGE_TOP + 2, 35, 20, hwndParent, NULL, g_hInstance, NULL);
    g_hEdCutQty = CreateWindow("EDIT", "1", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER, PAGE_MARGIN + 630, PAGE_TOP, 40, 22, hwndParent, (HMENU)IDC_ED_CUT_QTY, g_hInstance, NULL);
    
    g_hBtnAddCut = CreateWindow("BUTTON", "Adicionar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON, PAGE_MARGIN + 680, PAGE_TOP - 1, 90, 24, hwndParent, (HMENU)IDC_BTN_ADD_CUT, g_hInstance, NULL);
    
    RECT parentRc;
    GetClientRect(hwndParent, &parentRc);
    int listWidth = 360;
    int listHeight = parentRc.bottom - PAGE_TOP - 125;
    int canvasWidth = parentRc.right - listWidth - PAGE_MARGIN * 3;
    int canvasHeight = parentRc.bottom - PAGE_TOP - 45;
    
    g_hListCuts = CreateWindowEx(WS_EX_CLIENTEDGE, WC_LISTVIEW, "", WS_CHILD | WS_VISIBLE | LVS_REPORT | WS_BORDER,
        PAGE_MARGIN, PAGE_TOP + 35, listWidth, listHeight, hwndParent, (HMENU)IDC_LIST_CUTS, g_hInstance, NULL);
    ListView_SetExtendedListViewStyle(g_hListCuts, LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES);
    
    LVCOLUMNA lvc;
    ZeroMemory(&lvc, sizeof(lvc));
    lvc.mask = LVCF_TEXT | LVCF_WIDTH;
    lvc.pszText = "Largura"; lvc.cx = 70; ListView_InsertColumn(g_hListCuts, 0, &lvc);
    lvc.pszText = "Altura"; lvc.cx = 70; ListView_InsertColumn(g_hListCuts, 1, &lvc);
    lvc.pszText = "Esp"; lvc.cx = 50; ListView_InsertColumn(g_hListCuts, 2, &lvc);
    lvc.pszText = "Material"; lvc.cx = 80; ListView_InsertColumn(g_hListCuts, 3, &lvc);
    lvc.pszText = "Qtd"; lvc.cx = 50; ListView_InsertColumn(g_hListCuts, 4, &lvc);
    
    int buttonY = PAGE_TOP + 35 + listHeight + 10;
    g_hBtnOptimize = CreateWindow("BUTTON", "OTIMIZAR CORTES", WS_CHILD | BS_PUSHBUTTON,
        PAGE_MARGIN, buttonY, 150, 30, hwndParent, (HMENU)IDC_BTN_OPTIMIZE, g_hInstance, NULL);
    
    g_hBtnResetCuts = CreateWindow("BUTTON", "Limpar Lista", WS_CHILD | BS_PUSHBUTTON,
        PAGE_MARGIN + 160, buttonY, 100, 30, hwndParent, (HMENU)IDC_BTN_RESET_CUTS, g_hInstance, NULL);
    
    g_hBtnConfirmCuts = CreateWindow("BUTTON", "CONFIRMAR", WS_CHILD | BS_PUSHBUTTON,
        PAGE_MARGIN + 270, buttonY, 140, 30, hwndParent, (HMENU)IDC_BTN_CONFIRM_CUTS, g_hInstance, NULL);
    
    g_hCanvasResult = CreateWindowEx(WS_EX_CLIENTEDGE, "CanvasWindow", "", WS_CHILD | WS_VISIBLE | WS_BORDER | WS_VSCROLL,
        PAGE_MARGIN * 2 + listWidth + 10, PAGE_TOP + 35, canvasWidth - 10, canvasHeight, hwndParent, (HMENU)IDC_ED_RESULT, g_hInstance, NULL);
}

static void CreateEstadoPage(HWND hwndParent) {
    g_hCapProxyUrl = CreateWindow("STATIC", "Proxy URL:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN, PAGE_TOP - 20, 80, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hEdProxyUrl = CreateWindowEx(WS_EX_CLIENTEDGE, "EDIT", "", WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL, PAGE_MARGIN + 85, PAGE_TOP - 22, 300, 20, hwndParent, (HMENU)IDC_ED_PROXY_URL, g_hInstance, NULL);
    g_hBtnSaveProxy = CreateWindow("BUTTON", "Guardar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON, PAGE_MARGIN + 395, PAGE_TOP - 23, 70, 22, hwndParent, (HMENU)IDC_BTN_SAVE_PROXY, g_hInstance, NULL);
    
    g_hCapProxy = CreateWindow("STATIC", "Proxy:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN, PAGE_TOP + 4, 50, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblProxy = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 60, PAGE_TOP + 4, 200, 18, hwndParent, (HMENU)IDC_LBL_PROXY, g_hInstance, NULL);
    g_hCapMain = CreateWindow("STATIC", "Main server:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN, PAGE_TOP + 28, 80, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblMain = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 90, PAGE_TOP + 28, 200, 18, hwndParent, (HMENU)IDC_LBL_MAIN, g_hInstance, NULL);
    g_hCapUptime = CreateWindow("STATIC", "Uptime:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN, PAGE_TOP + 52, 60, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblUptime = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 70, PAGE_TOP + 52, 200, 18, hwndParent, (HMENU)IDC_LBL_UPTIME, g_hInstance, NULL);
    g_hCapDb = CreateWindow("STATIC", "DB:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN, PAGE_TOP + 76, 40, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblDb = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 50, PAGE_TOP + 76, 400, 18, hwndParent, (HMENU)IDC_LBL_DB, g_hInstance, NULL);
    g_hCapPending = CreateWindow("STATIC", "Pendentes:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN, PAGE_TOP + 100, 70, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hLblPending = CreateWindow("STATIC", "?", WS_CHILD | WS_VISIBLE, PAGE_MARGIN + 80, PAGE_TOP + 100, 200, 18, hwndParent, (HMENU)IDC_LBL_PENDING, g_hInstance, NULL);
    g_hBtnStatus = CreateWindow("BUTTON", "Atualizar estado", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON, PAGE_MARGIN, PAGE_TOP + 130, 130, 24, hwndParent, (HMENU)IDC_BTN_STATUS, g_hInstance, NULL);
    
    g_hCapFontSize = CreateWindow("STATIC", "Tamanho do texto:", WS_CHILD | WS_VISIBLE, PAGE_MARGIN, PAGE_TOP + 170, 120, 18, hwndParent, NULL, g_hInstance, NULL);
    g_hSliderFont = CreateWindowEx(0, TRACKBAR_CLASS, "", WS_CHILD | WS_VISIBLE | TBS_AUTOTICKS | TBS_TOOLTIPS, 
                                   PAGE_MARGIN + 130, PAGE_TOP + 168, 200, 24, hwndParent, (HMENU)IDC_SLIDER_FONTSIZE, g_hInstance, NULL);
    SendMessage(g_hSliderFont, TBM_SETRANGE, TRUE, MAKELONG(8, 24));
    SendMessage(g_hSliderFont, TBM_SETPOS, TRUE, g_fontSize);
    SendMessage(g_hSliderFont, TBM_SETTICFREQ, 2, 0);
    
    g_hChkAutoRefresh = CreateWindow("BUTTON", "Atualizar automaticamente (30s)", WS_CHILD | WS_VISIBLE | BS_AUTOCHECKBOX,
                                     PAGE_MARGIN, PAGE_TOP + 200, 220, 20, hwndParent, (HMENU)IDC_CHK_AUTOREFRESH, g_hInstance, NULL);
    
    LoadProxyUrl();
}

static void SwitchTab(int idx) {
    BOOL showRet = (idx==0), showOpt=(idx==1), showCut=(idx==2), showSt=(idx==3);
    ShowWindow(GetDlgItem(g_hMainWindow, IDC_BTN_ADD), showRet?SW_SHOW:SW_HIDE);
    ShowWindow(GetDlgItem(g_hMainWindow, IDC_BTN_REMOVE), showRet?SW_SHOW:SW_HIDE);
    ShowWindow(GetDlgItem(g_hMainWindow, IDC_BTN_REFRESH), showRet?SW_SHOW:SW_HIDE);
    ShowWindow(g_hListView, showRet?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblWCap, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdW, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblHCap, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdH, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblTCap, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdT, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblMatCap, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdMat, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hBtnFind, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hListSearch, showOpt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblCutWCap, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdCutW, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblCutHCap, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdCutH, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblCutTCap, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdCutT, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblCutMatCap, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdCutMat, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblCutQtyCap, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdCutQty, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hBtnAddCut, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hListCuts, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hBtnOptimize, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hBtnResetCuts, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hBtnConfirmCuts, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hCanvasResult, showCut?SW_SHOW:SW_HIDE);
    ShowWindow(g_hCapProxyUrl, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hEdProxyUrl, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hBtnSaveProxy, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hCapProxy, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblProxy, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hCapMain, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblMain, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hCapUptime, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblUptime, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hCapDb, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblDb, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hCapPending, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hLblPending, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hBtnStatus, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hCapFontSize, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hSliderFont, showSt?SW_SHOW:SW_HIDE);
    ShowWindow(g_hChkAutoRefresh, showSt?SW_SHOW:SW_HIDE);
}

static void DoSearch(void) {
    char w[16], h[16], t[16], m[64];
    GetWindowTextA(g_hEdW, w, sizeof(w));
    GetWindowTextA(g_hEdH, h, sizeof(h));
    GetWindowTextA(g_hEdT, t, sizeof(t));
    GetWindowTextA(g_hEdMat, m, sizeof(m));
    if (!w[0]||!h[0]||!t[0]||!m[0]) { MessageBox(g_hMainWindow, "Preencha todos os campos", "Pesquisar", MB_OK|MB_ICONWARNING); return; }
    char path[256]; wsprintfA(path, "/search?width_mm=%d&height_mm=%d&thickness_mm=%d&material=%s", atoi(w), atoi(h), atoi(t), m);
    char* resp=NULL; 
    if (!HttpRequestEx("GET",path,NULL,&resp)) { 
        MessageBox(g_hMainWindow,"Falha na requisicao","Erro",MB_OK|MB_ICONERROR); 
        return; 
    }
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
    WNDCLASSEXA wc; 
    ZeroMemory(&wc, sizeof(wc)); 
    wc.cbSize = sizeof(wc); 
    wc.lpfnWndProc = AddWndProc; 
    wc.hInstance = g_hInstance; 
    wc.hCursor = LoadCursor(NULL, IDC_ARROW); 
    wc.hbrBackground = (HBRUSH)(COLOR_BTNFACE+1); 
    wc.lpszClassName = "AddDialogClass";
    RegisterClassExA(&wc);
    
    HWND dlg = CreateWindowExA(WS_EX_DLGMODALFRAME, "AddDialogClass", "Adicionar Resto", WS_POPUP|WS_CAPTION|WS_SYSMENU,
        CW_USEDEFAULT,CW_USEDEFAULT,280,190, owner, NULL, g_hInstance, NULL);
    if (!dlg) return; ShowWindow(dlg, SW_SHOW); UpdateWindow(dlg);
    MSG m; BOOL running=TRUE; while(running && GetMessage(&m,NULL,0,0)){ if(!IsDialogMessage(dlg,&m)){ TranslateMessage(&m); DispatchMessage(&m);} if(!IsWindow(dlg)) running=FALSE; }
    RefreshListView();
}

static LRESULT CALLBACK EditWndProc(HWND hDlg, UINT uMsg, WPARAM wParam, LPARAM lParam) {
    static HWND eMat,eW,eH,eT,eNotes;
    static int editId;
    switch(uMsg){
        case WM_CREATE: {
            CREATESTRUCT* cs = (CREATESTRUCT*)lParam;
            editId = (int)cs->lpCreateParams;
            
            Resto* resto = NULL;
            for (int i = 0; i < g_inventoryCount; i++) {
                if (g_inventory[i].id == editId) {
                    resto = &g_inventory[i];
                    break;
                }
            }
            if (!resto) { DestroyWindow(hDlg); return 0; }
            
            CreateWindow("STATIC","Material:",WS_CHILD|WS_VISIBLE,10,12,60,18,hDlg,NULL,g_hInstance,NULL);
            eMat=CreateWindow("EDIT","",WS_CHILD|WS_VISIBLE|WS_BORDER,80,10,180,20,hDlg,NULL,g_hInstance,NULL);
            SetWindowTextA(eMat, resto->material);
            
            CreateWindow("STATIC","Largura:",WS_CHILD|WS_VISIBLE,10,40,60,18,hDlg,NULL,g_hInstance,NULL);
            eW=CreateWindow("EDIT","",WS_CHILD|WS_VISIBLE|WS_BORDER|ES_NUMBER,80,38,60,20,hDlg,NULL,g_hInstance,NULL);
            char buf[32];
            wsprintfA(buf, "%d", resto->width_mm);
            SetWindowTextA(eW, buf);
            
            CreateWindow("STATIC","Altura:",WS_CHILD|WS_VISIBLE,150,40,50,18,hDlg,NULL,g_hInstance,NULL);
            eH=CreateWindow("EDIT","",WS_CHILD|WS_VISIBLE|WS_BORDER|ES_NUMBER,205,38,55,20,hDlg,NULL,g_hInstance,NULL);
            wsprintfA(buf, "%d", resto->height_mm);
            SetWindowTextA(eH, buf);
            
            CreateWindow("STATIC","Espessura:",WS_CHILD|WS_VISIBLE,10,68,65,18,hDlg,NULL,g_hInstance,NULL);
            eT=CreateWindow("EDIT","",WS_CHILD|WS_VISIBLE|WS_BORDER|ES_NUMBER,80,66,60,20,hDlg,NULL,g_hInstance,NULL);
            wsprintfA(buf, "%d", resto->thickness_mm);
            SetWindowTextA(eT, buf);
            
            CreateWindow("STATIC","Notas:",WS_CHILD|WS_VISIBLE,10,96,50,18,hDlg,NULL,g_hInstance,NULL);
            eNotes=CreateWindow("EDIT","",WS_CHILD|WS_VISIBLE|WS_BORDER|ES_AUTOHSCROLL,80,94,180,20,hDlg,NULL,g_hInstance,NULL);
            SetWindowTextA(eNotes, resto->notes);
            
            CreateWindow("BUTTON","OK",WS_CHILD|WS_VISIBLE|BS_DEFPUSHBUTTON,60,125,80,24,hDlg,(HMENU)IDOK,g_hInstance,NULL);
            CreateWindow("BUTTON","Cancelar",WS_CHILD|WS_VISIBLE|BS_PUSHBUTTON,150,125,80,24,hDlg,(HMENU)IDCANCEL,g_hInstance,NULL);
            return 0;
        }
        case WM_COMMAND:
            if (LOWORD(wParam)==IDOK){ 
                char mat[64],w[16],h[16],t[16],notes[256];
                GetWindowTextA(eMat,mat,sizeof(mat)); 
                GetWindowTextA(eW,w,sizeof(w)); 
                GetWindowTextA(eH,h,sizeof(h)); 
                GetWindowTextA(eT,t,sizeof(t)); 
                GetWindowTextA(eNotes,notes,sizeof(notes));
                
                if(!mat[0]||!w[0]||!h[0]||!t[0]){ 
                    MessageBox(hDlg,"Campos obrigatorios faltando","Editar",MB_OK|MB_ICONWARNING); 
                    return 0; 
                }
                
                char path[64];
                wsprintfA(path, "/remove/%d", editId);
                if (!HttpRequestEx("DELETE", path, NULL, NULL)) { 
                    MessageBox(hDlg,"Erro ao atualizar","Erro",MB_OK|MB_ICONERROR);
                    return 0;
                }
                
                char json[640];
                if (notes[0]) 
                    wsprintfA(json,"{\"width_mm\":%d,\"height_mm\":%d,\"thickness_mm\":%d,\"material\":\"%s\",\"notes\":\"%s\"}",atoi(w),atoi(h),atoi(t),mat,notes);
                else 
                    wsprintfA(json,"{\"width_mm\":%d,\"height_mm\":%d,\"thickness_mm\":%d,\"material\":\"%s\",\"notes\":null}",atoi(w),atoi(h),atoi(t),mat);
                
                char* resp=NULL; 
                if (HttpRequestEx("POST","/add",json,&resp)){ 
                    if(resp) free(resp); 
                    DestroyWindow(hDlg);
                } else { 
                    MessageBox(hDlg,"Falha ao atualizar","Erro",MB_OK|MB_ICONERROR);
                } 
                return 0; 
            }
            if (LOWORD(wParam)==IDCANCEL){ DestroyWindow(hDlg); return 0; }
            break;
        case WM_CLOSE: DestroyWindow(hDlg); return 0;
    }
    return DefWindowProc(hDlg,uMsg,wParam,lParam);
}

static void ShowEditDialog(HWND owner, int restoId){
    WNDCLASSEXA wc; 
    ZeroMemory(&wc, sizeof(wc)); 
    wc.cbSize = sizeof(wc); 
    wc.lpfnWndProc = EditWndProc; 
    wc.hInstance = g_hInstance; 
    wc.hCursor = LoadCursor(NULL, IDC_ARROW); 
    wc.hbrBackground = (HBRUSH)(COLOR_BTNFACE+1); 
    wc.lpszClassName = "EditDialogClass";
    RegisterClassExA(&wc);
    
    HWND dlg = CreateWindowExA(WS_EX_DLGMODALFRAME, "EditDialogClass", "Editar Resto", WS_POPUP|WS_CAPTION|WS_SYSMENU,
        CW_USEDEFAULT,CW_USEDEFAULT,280,190, owner, NULL, g_hInstance, (LPVOID)restoId);
    if (!dlg) return; ShowWindow(dlg, SW_SHOW); UpdateWindow(dlg);
    MSG m; BOOL running=TRUE; while(running && GetMessage(&m,NULL,0,0)){ if(!IsDialogMessage(dlg,&m)){ TranslateMessage(&m); DispatchMessage(&m);} if(!IsWindow(dlg)) running=FALSE; }
    RefreshListView();
}

LRESULT CALLBACK WindowProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam) {
    switch(uMsg){
        case WM_CREATE: {
            ParseProxyUrl(); RECT rc; GetClientRect(hwnd,&rc);
            g_hTab = CreateWindowEx(0, WC_TABCONTROL, "", WS_CHILD|WS_VISIBLE|WS_CLIPSIBLINGS, 0,0, rc.right, 44, hwnd, (HMENU)IDC_TAB, g_hInstance, NULL);
            TCITEMA ti; ZeroMemory(&ti,sizeof(ti)); ti.mask=TCIF_TEXT; ti.pszText="Retalhos"; TabCtrl_InsertItem(g_hTab,0,&ti); ti.pszText="Pesquisa"; TabCtrl_InsertItem(g_hTab,1,&ti); ti.pszText="Corte"; TabCtrl_InsertItem(g_hTab,2,&ti); ti.pszText="Estado"; TabCtrl_InsertItem(g_hTab,3,&ti);
            CreateRetalhosPage(hwnd); CreateOtimizarPage(hwnd); CreateCortePage(hwnd); CreateEstadoPage(hwnd); 
            LoadUISettings();
            CreateAppFont(); ApplyFontToAllControls(); SwitchTab(0);
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
                case IDC_BTN_SAVE_PROXY: SaveProxyUrl(); return 0;
                case IDC_CHK_AUTOREFRESH: {
                    g_autoRefresh = (SendMessage(g_hChkAutoRefresh, BM_GETCHECK, 0, 0) == BST_CHECKED);
                    if (g_autoRefresh) {
                        SetTimer(hwnd, TIMER_AUTOREFRESH, AUTOREFRESH_INTERVAL, NULL);
                    } else {
                        KillTimer(hwnd, TIMER_AUTOREFRESH);
                    }
                    return 0;
                }
                case IDC_BTN_ADD_CUT: AddCutToPlan(); return 0;
                case IDC_BTN_OPTIMIZE: DoCutOptimization(); return 0;
                case IDC_BTN_RESET_CUTS: 
                    ListView_DeleteAllItems(g_hListCuts);
                    g_optimizeResult[0] = '\0';
                    InvalidateRect(g_hCanvasResult, NULL, TRUE);
                    return 0;
                case IDC_BTN_CONFIRM_CUTS:
                    ConfirmCuttingPlan();
                    return 0;
            }
            break;
        case WM_NOTIFY: {
            LPNMHDR pnmh = (LPNMHDR)lParam;
            if (wParam==IDC_TAB && pnmh->code==TCN_SELCHANGE){ 
                int idx=TabCtrl_GetCurSel(g_hTab); 
                SwitchTab(idx); 
                return 0; 
            }
            if (wParam==IDC_LISTVIEW && pnmh->code==NM_DBLCLK) {
                LPNMITEMACTIVATE lpnmitem = (LPNMITEMACTIVATE)lParam;
                if (lpnmitem->iItem >= 0 && lpnmitem->iItem < g_inventoryCount) {
                    int restoId = g_inventory[lpnmitem->iItem].id;
                    ShowEditDialog(hwnd, restoId);
                }
                return 0;
            }
            if ((wParam==IDC_LISTVIEW || wParam==IDC_LISTSEARCH) && pnmh->code==NM_CUSTOMDRAW) {
                LPNMLVCUSTOMDRAW pcd = (LPNMLVCUSTOMDRAW)lParam;
                if (pcd->nmcd.dwDrawStage == CDDS_PREPAINT) {
                    return CDRF_NOTIFYITEMDRAW;
                }
                if (pcd->nmcd.dwDrawStage == CDDS_ITEMPREPAINT) {
                    if (g_hListViewFont) SelectObject(pcd->nmcd.hdc, g_hListViewFont);
                    return CDRF_NEWFONT;
                }
            }
            break;
        }
        case WM_TIMER:
            if (wParam == TIMER_AUTOREFRESH) {
                RefreshListView();
                return 0;
            }
            break;
        case WM_HSCROLL:
            if ((HWND)lParam == g_hSliderFont) {
                g_fontSize = (int)SendMessage(g_hSliderFont, TBM_GETPOS, 0, 0);
                CreateAppFont();
                ApplyFontToAllControls();
                SaveUISettings();
                return 0;
            }
            break;
        case WM_SIZE: {
            SendMessage(g_hStatusBar, WM_SIZE, 0, 0);
            if (g_hTab) MoveWindow(g_hTab, 0, 0, LOWORD(lParam), TAB_HEIGHT, TRUE);
            if (g_hListView) SetWindowPos(g_hListView,NULL, PAGE_MARGIN, PAGE_TOP + ROW_GAP, LOWORD(lParam) - 2*PAGE_MARGIN, HIWORD(lParam) - (PAGE_TOP + ROW_GAP) - 46, SWP_NOZORDER);
            if (g_hListSearch) SetWindowPos(g_hListSearch,NULL, PAGE_MARGIN, PAGE_TOP + ROW_GAP, LOWORD(lParam) - 2*PAGE_MARGIN, HIWORD(lParam) - (PAGE_TOP + ROW_GAP) - 46, SWP_NOZORDER);
            return 0; }
        case WM_DESTROY: 
            KillTimer(hwnd, TIMER_AUTOREFRESH);
            SaveUISettings();
            if (g_hAppFont) DeleteObject(g_hAppFont);
            if (g_hListViewFont) DeleteObject(g_hListViewFont);
            PostQuitMessage(0); 
            return 0;
    }
    return DefWindowProc(hwnd,uMsg,wParam,lParam);
}

LRESULT CALLBACK CanvasWndProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam) {
    switch(uMsg) {
        case WM_VSCROLL: {
            SCROLLINFO si;
            ZeroMemory(&si, sizeof(si));
            si.cbSize = sizeof(si);
            si.fMask = SIF_ALL;
            GetScrollInfo(hwnd, SB_VERT, &si);
            
            int yPos = si.nPos;
            switch(LOWORD(wParam)) {
                case SB_LINEUP: yPos -= 20; break;
                case SB_LINEDOWN: yPos += 20; break;
                case SB_PAGEUP: yPos -= si.nPage; break;
                case SB_PAGEDOWN: yPos += si.nPage; break;
                case SB_THUMBTRACK: yPos = si.nTrackPos; break;
            }
            
            if (yPos < 0) yPos = 0;
            if (yPos > si.nMax - (int)si.nPage + 1) yPos = si.nMax - si.nPage + 1;
            if (yPos < 0) yPos = 0;
            
            si.fMask = SIF_POS;
            si.nPos = yPos;
            SetScrollInfo(hwnd, SB_VERT, &si, TRUE);
            g_canvasScrollPos = yPos;
            
            InvalidateRect(hwnd, NULL, TRUE);
            return 0;
        }
        case WM_PAINT: {
            PAINTSTRUCT ps;
            HDC hdc = BeginPaint(hwnd, &ps);
            RECT rc;
            GetClientRect(hwnd, &rc);
            
            FillRect(hdc, &rc, (HBRUSH)(COLOR_WINDOW+1));
            
            // Check if we have valid JSON
            if (g_optimizeResult[0] == '{') {
                
                // 1. Parse Header Info
                int totalPlaced = GetJsonInt(g_optimizeResult, "\"total_cuts_placed\"");
                int totalRequested = GetJsonInt(g_optimizeResult, "\"total_cuts_requested\"");
                float efficiency = GetJsonFloat(g_optimizeResult, "\"efficiency_percent\"");
                
                char header[256];
                // Manual float formatting for C (e.g. 76.2%)
                wsprintfA(header, "Eficiencia: %d.%d%%  |  Pecas: %d de %d", 
                          (int)efficiency, (int)((efficiency - (int)efficiency)*10), 
                          totalPlaced, totalRequested);
                
                // Draw Header
                RECT headerRc = {0, 0, rc.right, 30};
                FillRect(hdc, &headerRc, (HBRUSH)(COLOR_BTNFACE+1));
                SetBkMode(hdc, TRANSPARENT);
                DrawText(hdc, header, -1, &headerRc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                
                int yOffset = 35 - g_canvasScrollPos;
                int totalHeight = 35;

                // 2. Parse Used Planks
                const char* p = strstr(g_optimizeResult, "\"used_planks\":[");
                if (p) {
                    p += 15; // Skip key
                    int plankNum = 1;
                    int safeGuard = 0; 

                    while (*p && *p != ']' && safeGuard < 500) {
                        safeGuard++;
                        
                        // Find start of plank object
                        const char* plankStart = strchr(p, '{');
                        if (!plankStart) break;
                        
                        // Find end of plank object using the BRACE COUNTING fix
                        const char* plankEnd = GetJsonObjectEnd(plankStart);
                        if (!plankEnd) break;

                        // Parse Plank Data
                        int restoId = GetJsonInt(plankStart, "\"resto_id\"");
                        int width = GetJsonInt(plankStart, "\"width_mm\"");
                        int height = GetJsonInt(plankStart, "\"height_mm\"");

                        // Draw Plank Title
                        char plankInfo[128];
                        wsprintfA(plankInfo, "Prancha #%d (ID:%d) - %dx%dmm", plankNum++, restoId, width, height);
                        
                        SetTextColor(hdc, RGB(0,0,0));
                        TextOut(hdc, 10, yOffset, plankInfo, strlen(plankInfo));
                        yOffset += 20;

                        // Calculate Scale
                        float scale = 0.3f;
                        if (width > 0) {
                            int maxW = rc.right - 40;
                            if (maxW < 100) maxW = 100;
                            float scaleW = (float)maxW / width;
                            if (scaleW < scale) scale = scaleW;
                        }
                        
                        int drawW = (int)(width * scale);
                        int drawH = (int)(height * scale);
                        
                        RECT plankRc = {20, yOffset, 20 + drawW, yOffset + drawH};
                        
                        // Draw Plank Rect
                        HBRUSH hBrPlank = CreateSolidBrush(RGB(220, 220, 220));
                        FillRect(hdc, &plankRc, hBrPlank);
                        DeleteObject(hBrPlank);
                        FrameRect(hdc, &plankRc, (HBRUSH)GetStockObject(BLACK_BRUSH));

                        // 3. Draw Cuts inside this plank
                        // Limit the search for "cuts" strictly to the range of [plankStart, plankEnd]
                        const char* cuts = strstr(plankStart, "\"cuts\":[");
                        if (cuts && cuts < plankEnd) {
                            const char* c = cuts;
                            const char* cutsEnd = strchr(cuts, ']');
                            // Ensure we don't overshoot the plank end
                            if (cutsEnd > plankEnd) cutsEnd = plankEnd;
                            
                            HBRUSH hBrCut = CreateSolidBrush(RGB(100, 150, 255));
                            
                            while (c && c < cutsEnd) {
                                const char* cutStart = strchr(c, '{');
                                if (!cutStart || cutStart > cutsEnd) break;
                                
                                const char* cutEnd = GetJsonObjectEnd(cutStart);
                                if (!cutEnd) break;
                                
                                int cx = GetJsonInt(cutStart, "\"x\"");
                                int cy = GetJsonInt(cutStart, "\"y\"");
                                int cw = GetJsonInt(cutStart, "\"width\"");
                                int ch = GetJsonInt(cutStart, "\"height\"");
                                
                                if (cw > 0 && ch > 0) {
                                    RECT cutRc;
                                    cutRc.left = plankRc.left + (int)(cx * scale);
                                    cutRc.top = plankRc.top + (int)(cy * scale);
                                    cutRc.right = cutRc.left + (int)(cw * scale);
                                    cutRc.bottom = cutRc.top + (int)(ch * scale);
                                    
                                    FillRect(hdc, &cutRc, hBrCut);
                                    FrameRect(hdc, &cutRc, (HBRUSH)GetStockObject(BLACK_BRUSH));

                                    int pxWidth = cutRc.right - cutRc.left;
                                    int pxHeight = cutRc.bottom - cutRc.top;
                                    
                                    // Draw Dimensions
                                    if (cw >= 30 && ch >= 15 && pxWidth > 25 && pxHeight > 10) {
                                        char dimStr[32];
                                        wsprintfA(dimStr, "%dx%d", cw, ch);
                                        SetBkMode(hdc, TRANSPARENT);
                                        DrawText(hdc, dimStr, -1, &cutRc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                                    }
                                }
                                c = cutEnd + 1;
                            }
                            DeleteObject(hBrCut);
                        }

                        yOffset += drawH + 30;
                        p = plankEnd + 1; // Move loop to after the current plank
                    }
                }
                
                totalHeight = yOffset + g_canvasScrollPos;
                
                // Update Scrollbar
                if (totalHeight != g_canvasTotalHeight) {
                    g_canvasTotalHeight = totalHeight;
                    SCROLLINFO si;
                    si.cbSize = sizeof(si);
                    si.fMask = SIF_RANGE | SIF_PAGE;
                    si.nMin = 0;
                    si.nMax = totalHeight;
                    si.nPage = rc.bottom;
                    SetScrollInfo(hwnd, SB_VERT, &si, TRUE);
                }

            } else {
                char msg[] = "Execute a otimizacao para ver o plano de corte";
                DrawText(hdc, msg, -1, &rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }
            
            EndPaint(hwnd, &ps);
            return 0;
        }
    }
    return DefWindowProc(hwnd, uMsg, wParam, lParam);
}

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE hPrev, LPSTR lpCmdLine, int nCmdShow){
    g_hInstance = hInstance; INITCOMMONCONTROLSEX icc; icc.dwSize=sizeof(icc); icc.dwICC=ICC_LISTVIEW_CLASSES|ICC_BAR_CLASSES|ICC_TAB_CLASSES; InitCommonControlsEx(&icc);
    
    WNDCLASSEXA canvasWc;
    ZeroMemory(&canvasWc, sizeof(canvasWc));
    canvasWc.cbSize = sizeof(canvasWc);
    canvasWc.lpfnWndProc = CanvasWndProc;
    canvasWc.hInstance = hInstance;
    canvasWc.hCursor = LoadCursor(NULL, IDC_ARROW);
    canvasWc.hbrBackground = (HBRUSH)(COLOR_WINDOW+1);
    canvasWc.lpszClassName = "CanvasWindow";
    RegisterClassExA(&canvasWc);
    
    WNDCLASSEXA wc; ZeroMemory(&wc,sizeof(wc)); wc.cbSize=sizeof(wc); wc.lpfnWndProc=WindowProc; wc.hInstance=hInstance; wc.hCursor=LoadCursor(NULL,IDC_ARROW); wc.hbrBackground=(HBRUSH)(COLOR_3DFACE+1); wc.lpszClassName="RetListerWindowClass"; RegisterClassExA(&wc);
    g_hMainWindow = CreateWindowExA(0, "RetListerWindowClass", "RetLister - Gestao de Retalhos", WS_OVERLAPPEDWINDOW, CW_USEDEFAULT,CW_USEDEFAULT, WINDOW_WIDTH, WINDOW_HEIGHT, NULL, NULL, hInstance, NULL);
    if (!g_hMainWindow){ MessageBox(NULL,"Failed to create window","Error",MB_OK|MB_ICONERROR); return 1; }
    ShowWindow(g_hMainWindow, SW_MAXIMIZE); UpdateWindow(g_hMainWindow);
    MSG msg; while(GetMessage(&msg,NULL,0,0)){ TranslateMessage(&msg); DispatchMessage(&msg);} return (int)msg.wParam;
}
