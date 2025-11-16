#define WIN32_LEAN_AND_MEAN
#define _WIN32_WINNT 0x0501  // Windows XP
#include <windows.h>
#include <commctrl.h>
#include <wininet.h>
#include <shlwapi.h>
#include <stdio.h>
#include <stdlib.h>
#include <string>
#include <vector>

#pragma comment(lib, "comctl32.lib")
#pragma comment(lib, "wininet.lib")
#pragma comment(lib, "shlwapi.lib")

// Configuration
#define PROXY_URL "http://192.168.56.1:8001"
#define WINDOW_WIDTH 1000
#define WINDOW_HEIGHT 600

// Control IDs
#define IDC_LISTVIEW 1001
#define IDC_BTN_ADD 1002
#define IDC_BTN_REMOVE 1003
#define IDC_BTN_SEARCH 1004
#define IDC_BTN_REFRESH 1005
#define IDC_STATUSBAR 1006

// Global variables
HWND g_hMainWindow = NULL;
HWND g_hListView = NULL;
HWND g_hStatusBar = NULL;
HINSTANCE g_hInstance = NULL;

struct Resto {
    int id;
    int width_mm;
    int height_mm;
    int thickness_mm;
    std::string material;
    std::string notes;
    std::string created_at;
};

std::vector<Resto> g_inventory;

// HTTP helper function
std::string HttpRequest(const char* url, const char* method = "GET", const char* postData = NULL) {
    std::string result;
    HINTERNET hInternet = InternetOpenA("RetLister/1.0", INTERNET_OPEN_TYPE_DIRECT, NULL, NULL, 0);
    if (!hInternet) return result;

    HINTERNET hConnect = InternetOpenUrlA(hInternet, url, NULL, 0, INTERNET_FLAG_RELOAD, 0);
    if (hConnect) {
        char buffer[4096];
        DWORD bytesRead;
        while (InternetReadFile(hConnect, buffer, sizeof(buffer) - 1, &bytesRead) && bytesRead > 0) {
            buffer[bytesRead] = '\0';
            result += buffer;
        }
        InternetCloseHandle(hConnect);
    }
    InternetCloseHandle(hInternet);
    return result;
}

// Simple JSON parser for our specific format
void ParseRestoList(const char* json, std::vector<Resto>& restos) {
    restos.clear();
    const char* p = json;
    
    while (*p) {
        // Find next object
        p = strstr(p, "{\"id\":");
        if (!p) break;
        
        Resto r = {0};
        
        // Parse id
        if (sscanf(p, "{\"id\":%d", &r.id) == 1) {
            // Parse width_mm
            const char* w = strstr(p, "\"width_mm\":");
            if (w) sscanf(w, "\"width_mm\":%d", &r.width_mm);
            
            // Parse height_mm
            const char* h = strstr(p, "\"height_mm\":");
            if (h) sscanf(h, "\"height_mm\":%d", &r.height_mm);
            
            // Parse thickness_mm
            const char* t = strstr(p, "\"thickness_mm\":");
            if (t) sscanf(t, "\"thickness_mm\":%d", &r.thickness_mm);
            
            // Parse material
            const char* m = strstr(p, "\"material\":\"");
            if (m) {
                m += 12;
                const char* end = strchr(m, '"');
                if (end) r.material.assign(m, end - m);
            }
            
            // Parse notes (may be null)
            const char* n = strstr(p, "\"notes\":");
            if (n) {
                n += 8;
                if (strncmp(n, "null", 4) != 0 && *n == '"') {
                    n++;
                    const char* end = strchr(n, '"');
                    if (end) r.notes.assign(n, end - n);
                }
            }
            
            restos.push_back(r);
        }
        
        p++;
    }
}

void RefreshListView() {
    ListView_DeleteAllItems(g_hListView);
    
    std::string url = std::string(PROXY_URL) + "/list";
    std::string response = HttpRequest(url.c_str());
    
    ParseRestoList(response.c_str(), g_inventory);
    
    for (size_t i = 0; i < g_inventory.size(); i++) {
        LVITEMA lvi = {0};
        lvi.mask = LVIF_TEXT;
        lvi.iItem = i;
        
        // ID column
        char buf[256];
        sprintf(buf, "#%d", g_inventory[i].id);
        lvi.pszText = buf;
        lvi.iSubItem = 0;
        int idx = ListView_InsertItem(g_hListView, &lvi);
        
        // Width
        sprintf(buf, "%d", g_inventory[i].width_mm);
        ListView_SetItemText(g_hListView, idx, 1, buf);
        
        // Height
        sprintf(buf, "%d", g_inventory[i].height_mm);
        ListView_SetItemText(g_hListView, idx, 2, buf);
        
        // Thickness
        sprintf(buf, "%d", g_inventory[i].thickness_mm);
        ListView_SetItemText(g_hListView, idx, 3, buf);
        
        // Material
        ListView_SetItemText(g_hListView, idx, 4, (char*)g_inventory[i].material.c_str());
        
        // Notes
        ListView_SetItemText(g_hListView, idx, 5, (char*)g_inventory[i].notes.c_str());
    }
    
    char status[256];
    sprintf(status, "%d retalhos", g_inventory.size());
    SendMessage(g_hStatusBar, SB_SETTEXT, 0, (LPARAM)status);
}

void CreateListView(HWND hwndParent) {
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
    
    // Set extended styles
    ListView_SetExtendedListViewStyle(g_hListView, LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES);
    
    // Add columns
    LVCOLUMNA lvc = {0};
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

void ShowAddDialog(HWND hwndParent) {
    // TODO: Create add dialog
    MessageBox(hwndParent, "Add dialog - coming soon", "Add", MB_OK);
}

void RemoveSelected() {
    int selected = ListView_GetNextItem(g_hListView, -1, LVNI_SELECTED);
    if (selected < 0 || selected >= (int)g_inventory.size()) {
        MessageBox(g_hMainWindow, "Sem selecao", "Remover", MB_OK | MB_ICONWARNING);
        return;
    }
    
    int id = g_inventory[selected].id;
    
    char msg[256];
    sprintf(msg, "Remover resto #%d?", id);
    if (MessageBox(g_hMainWindow, msg, "Confirmar", MB_YESNO | MB_ICONQUESTION) != IDYES) {
        return;
    }
    
    char url[512];
    sprintf(url, "%s/remove/%d", PROXY_URL, id);
    
    HINTERNET hInternet = InternetOpenA("RetLister/1.0", INTERNET_OPEN_TYPE_DIRECT, NULL, NULL, 0);
    if (hInternet) {
        HINTERNET hConnect = InternetOpenUrlA(hInternet, url, "DELETE", -1, INTERNET_FLAG_RELOAD, 0);
        if (hConnect) {
            InternetCloseHandle(hConnect);
            RefreshListView();
        }
        InternetCloseHandle(hInternet);
    }
}

LRESULT CALLBACK WindowProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam) {
    switch (uMsg) {
        case WM_CREATE:
            // Create toolbar buttons
            CreateWindow("BUTTON", "Adicionar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                10, 10, 100, 30, hwnd, (HMENU)IDC_BTN_ADD, g_hInstance, NULL);
            
            CreateWindow("BUTTON", "Remover", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                120, 10, 100, 30, hwnd, (HMENU)IDC_BTN_REMOVE, g_hInstance, NULL);
            
            CreateWindow("BUTTON", "Procurar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                230, 10, 100, 30, hwnd, (HMENU)IDC_BTN_SEARCH, g_hInstance, NULL);
            
            CreateWindow("BUTTON", "Atualizar", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                340, 10, 100, 30, hwnd, (HMENU)IDC_BTN_REFRESH, g_hInstance, NULL);
            
            // Create list view
            CreateListView(hwnd);
            
            // Create status bar
            g_hStatusBar = CreateWindowEx(0, STATUSCLASSNAME, NULL,
                WS_CHILD | WS_VISIBLE | SBARS_SIZEGRIP,
                0, 0, 0, 0, hwnd, (HMENU)IDC_STATUSBAR, g_hInstance, NULL);
            
            // Load initial data
            RefreshListView();
            break;
            
        case WM_COMMAND:
            switch (LOWORD(wParam)) {
                case IDC_BTN_ADD:
                    ShowAddDialog(hwnd);
                    break;
                case IDC_BTN_REMOVE:
                    RemoveSelected();
                    break;
                case IDC_BTN_SEARCH:
                    MessageBox(hwnd, "Search dialog - coming soon", "Search", MB_OK);
                    break;
                case IDC_BTN_REFRESH:
                    RefreshListView();
                    break;
            }
            break;
            
        case WM_SIZE:
            SendMessage(g_hStatusBar, WM_SIZE, 0, 0);
            if (g_hListView) {
                SetWindowPos(g_hListView, NULL, 10, 50, 
                    LOWORD(lParam) - 20, HIWORD(lParam) - 90, SWP_NOZORDER);
            }
            break;
            
        case WM_DESTROY:
            PostQuitMessage(0);
            break;
            
        default:
            return DefWindowProc(hwnd, uMsg, wParam, lParam);
    }
    return 0;
}

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, LPSTR lpCmdLine, int nCmdShow) {
    g_hInstance = hInstance;
    
    // Initialize common controls
    INITCOMMONCONTROLSEX icex;
    icex.dwSize = sizeof(INITCOMMONCONTROLSEX);
    icex.dwICC = ICC_LISTVIEW_CLASSES | ICC_BAR_CLASSES;
    InitCommonControlsEx(&icex);
    
    // Register window class
    WNDCLASSEXA wc = {0};
    wc.cbSize = sizeof(WNDCLASSEXA);
    wc.lpfnWndProc = WindowProc;
    wc.hInstance = hInstance;
    wc.hCursor = LoadCursor(NULL, IDC_ARROW);
    wc.hbrBackground = (HBRUSH)(COLOR_3DFACE + 1);
    wc.lpszClassName = "RetListerWindowClass";
    
    RegisterClassExA(&wc);
    
    // Create window
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
    
    // Message loop
    MSG msg;
    while (GetMessage(&msg, NULL, 0, 0)) {
        TranslateMessage(&msg);
        DispatchMessage(&msg);
    }
    
    return (int)msg.wParam;
}
