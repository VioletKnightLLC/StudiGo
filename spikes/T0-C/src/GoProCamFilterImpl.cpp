// GoProCamFilterImpl.cpp - Implementation of GoPro Virtual Camera Filter
// Simplified DirectShow source filter with color bars

#include "GoProCamFilter.h"
#include <strsafe.h>

#ifdef _DEBUG
#define DEBUG_LOG(x, ...) fprintf(stderr, "[GoProCamFilter] " x "\n", __VA_ARGS__)
#else
#define DEBUG_LOG(x, ...) 
#endif

// ============================================================================
// Module and DLL Exports
// ============================================================================

HMODULE g_hModule = nullptr;

BOOL APIENTRY DllMain(HINSTANCE hInstance, DWORD dwReason, LPVOID lpReserved)
{
    if (dwReason == DLL_PROCESS_ATTACH) {
        g_hModule = hInstance;
        DisableThreadLibraryCalls(hInstance);
        DEBUG_LOG("DLL loaded");
    } else if (dwReason == DLL_PROCESS_DETACH) {
        DEBUG_LOG("DLL unloaded");
    }
    return TRUE;
}

// COM Object DLL exports
STDAPI DllGetClassObject(REFCLSID rclsid, REFIID riid, LPVOID *ppv)
{
    if (!ppv) return E_POINTER;
    *ppv = nullptr;
    
    if (rclsid != CLSID_GoProCamFilter) {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    
    CGoProCamFilterFactory* pFactory = new (std::nothrow) CGoProCamFilterFactory();
    if (!pFactory) return E_OUTOFMEMORY;
    
    HRESULT hr = pFactory->QueryInterface(riid, ppv);
    pFactory->Release();
    return hr;
}

STDAPI DllCanUnloadNow()
{
    return S_FALSE;
}

STDAPI DllRegisterServer()
{
    return RegisterFilter();
}

STDAPI DllUnregisterServer()
{
    return UnregisterFilter();
}

// ============================================================================
// Class Factory Implementation
// ============================================================================

IFACEMETHODIMP CGoProCamFilterFactory::QueryInterface(REFIID riid, void **ppv)
{
    if (!ppv) return E_POINTER;
    
    if (riid == IID_IUnknown || riid == IID_IClassFactory) {
        *ppv = static_cast<IClassFactory*>(this);
        AddRef();
        return S_OK;
    }
    
    *ppv = nullptr;
    return E_NOINTERFACE;
}

IFACEMETHODIMP_(ULONG) CGoProCamFilterFactory::AddRef()
{
    return InterlockedIncrement(&m_cRef);
}

IFACEMETHODIMP_(ULONG) CGoProCamFilterFactory::Release()
{
    ULONG cRef = InterlockedDecrement(&m_cRef);
    if (cRef == 0) {
        delete this;
    }
    return cRef;
}

IFACEMETHODIMP CGoProCamFilterFactory::CreateInstance(IUnknown *pUnkOuter, REFIID riid, void **ppv)
{
    if (pUnkOuter) return CLASS_E_NOAGGREGATION;
    if (!ppv) return E_POINTER;
    
    *ppv = nullptr;
    
    CGoProCamFilter* pFilter = new (std::nothrow) CGoProCamFilter();
    if (!pFilter) return E_OUTOFMEMORY;
    
    HRESULT hr = pFilter->QueryInterface(riid, ppv);
    pFilter->Release();
    return hr;
}

IFACEMETHODIMP CGoProCamFilterFactory::LockServer(BOOL fLock)
{
    return S_OK;
}

// ============================================================================
// CGoProCamFilter Implementation
// ============================================================================

CGoProCamFilter::CGoProCamFilter()
    : m_cRef(1), m_State(State_Stopped), m_pClock(nullptr), m_pGraph(nullptr),
      m_pOutputPin(nullptr), m_pFrameBuffer(nullptr), m_FrameWidth(1920), 
      m_FrameHeight(1080), m_FrameStride(1920 * 4), m_rtFrameDuration(333333), 
      m_rtStartTime(0)
{
    // Initialize name
    StringCchCopyW(m_Name, MAX_FILTER_NAME, L"GoPro Webcam Studio");
    
    // Initialize filter info
    m_FilterInfo.achFilterName[0] = L'\0';
    m_FilterInfo.pGraph = nullptr;
    
    DEBUG_LOG("Filter created");
}

CGoProCamFilter::~CGoProCamFilter()
{
    if (m_pOutputPin) {
        delete m_pOutputPin;
    }
    if (m_pFrameBuffer) {
        CoTaskMemFree(m_pFrameBuffer);
    }
    DEBUG_LOG("Filter destroyed");
}

// IUnknown
IFACEMETHODIMP CGoProCamFilter::QueryInterface(REFIID riid, void **ppv)
{
    if (!ppv) return E_POINTER;
    
    if (riid == IID_IUnknown || riid == IID_IBaseFilter) {
        *ppv = static_cast<IBaseFilter*>(this);
        AddRef();
        return S_OK;
    }
    else if (riid == IID_IAMStreamConfig) {
        *ppv = static_cast<IAMStreamConfig*>(this);
        AddRef();
        return S_OK;
    }
    
    *ppv = nullptr;
    return E_NOINTERFACE;
}

IFACEMETHODIMP_(ULONG) CGoProCamFilter::AddRef()
{
    return InterlockedIncrement(&m_cRef);
}

IFACEMETHODIMP_(ULONG) CGoProCamFilter::Release()
{
    ULONG cRef = InterlockedDecrement(&m_cRef);
    if (cRef == 0) {
        delete this;
    }
    return cRef;
}

// IBaseFilter Implementation
IFACEMETHODIMP CGoProCamFilter::GetClassID(CLSID *pClsID)
{
    if (!pClsID) return E_POINTER;
    *pClsID = CLSID_GoProCamFilter;
    return S_OK;
}

IFACEMETHODIMP CGoProCamFilter::Stop()
{
    m_State = State_Stopped;
    return S_OK;
}

IFACEMETHODIMP CGoProCamFilter::Pause()
{
    m_State = State_Paused;
    return S_OK;
}

IFACEMETHODIMP CGoProCamFilter::Run(REFERENCE_TIME rtStart)
{
    m_rtStartTime = rtStart;
    m_State = State_Running;
    return S_OK;
}

IFACEMETHODIMP CGoProCamFilter::GetState(DWORD dwState, FILTER_STATE *pState)
{
    if (!pState) return E_POINTER;
    *pState = m_State;
    return S_OK;
}

IFACEMETHODIMP CGoProCamFilter::SetSyncSource(IReferenceClock *pClock)
{
    if (m_pClock) {
        m_pClock->Release();
    }
    m_pClock = pClock;
    if (m_pClock) {
        m_pClock->AddRef();
    }
    return S_OK;
}

IFACEMETHODIMP CGoProCamFilter::GetSyncSource(IReferenceClock **ppClock)
{
    if (!ppClock) return E_POINTER;
    *ppClock = m_pClock;
    if (*ppClock) {
        (*ppClock)->AddRef();
    }
    return S_OK;
}

IFACEMETHODIMP CGoProCamFilter::EnumPins(IEnumPins **ppEnum)
{
    if (!ppEnum) return E_POINTER;
    
    // Simple implementation - just return our single pin
    class CEnumPins : public IEnumPins
    {
    public:
        CEnumPins(CGoProCamFilter* pFilter) : m_cRef(1), m_pFilter(pFilter), m_position(0) {
            m_pFilter->AddRef();
        }
        
        // IUnknown
        IFACEMETHODIMP QueryInterface(REFIID riid, void **ppv) {
            if (riid == IID_IUnknown || riid == IID_IEnumPins) {
                *ppv = static_cast<IEnumPins*>(this);
                AddRef();
                return S_OK;
            }
            *ppv = nullptr;
            return E_NOINTERFACE;
        }
        IFACEMETHODIMP_(ULONG) AddRef() { return InterlockedIncrement(&m_cRef); }
        IFACEMETHODIMP_(ULONG) Release() {
            ULONG cRef = InterlockedDecrement(&m_cRef);
            if (cRef == 0) { m_pFilter->Release(); delete this; }
            return cRef;
        }
        
        // IEnumPins
        IFACEMETHODIMP Next(ULONG cPins, IPin **ppPins, ULONG *pcFetched) {
            if (!ppPins) return E_POINTER;
            if (m_position >= 1) return S_FALSE;
            
            // Return the output pin
            IPin* pPin = static_cast<IPin*>(m_pFilter->GetOutputPin());
            pPin->AddRef();
            ppPins[0] = pPin;
            m_position++;
            if (pcFetched) *pcFetched = 1;
            return S_OK;
        }
        IFACEMETHODIMP Skip(ULONG cPins) {
            m_position += cPins;
            return m_position <= 1 ? S_OK : S_FALSE;
        }
        IFACEMETHODIMP Reset() { m_position = 0; return S_OK; }
        IFACEMETHODIMP Clone(IEnumPins **ppEnum) { return E_NOTIMPL; }
        
    private:
        LONG m_cRef;
        CGoProCamFilter* m_pFilter;
        int m_position;
    };
    
    *ppEnum = new (std::nothrow) CEnumPins(this);
    return *ppEnum ? S_OK : E_OUTOFMEMORY;
}

IFACEMETHODIMP CGoProCamFilter::FindPin(LPCWSTR lpwstrPinId, IPin **ppPin)
{
    if (!lpwstrPinId || !ppPin) return E_POINTER;
    
    if (m_pOutputPin && wcscmp(lpwstrPinId, L"Output") == 0) {
        m_pOutputPin->AddRef();
        *ppPin = m_pOutputPin;
        return S_OK;
    }
    
    *ppPin = nullptr;
    return VFW_E_NOT_FOUND;
}

IFACEMETHODIMP CGoProCamFilter::QueryFilterInfo(FILTER_INFO *pInfo)
{
    if (!pInfo) return E_POINTER;
    
    // Copy filter name
    StringCchCopyW(pInfo->achFilterName, MAX_FILTER_NAME, m_Name);
    pInfo->pGraph = m_pGraph;
    if (pInfo->pGraph) {
        pInfo->pGraph->AddRef();
    }
    
    return S_OK;
}

IFACEMETHODIMP CGoProCamFilter::JoinFilterGraph(IFilterGraph *pGraph, LPCWSTR lpwstrName)
{
    m_pGraph = pGraph;
    if (lpwstrName) {
        StringCchCopyW(m_Name, MAX_FILTER_NAME, lpwstrName);
    }
    return S_OK;
}

IFACEMETHODIMP CGoProCamFilter::QueryVendorInfo(LPWSTR *pVendorInfo)
{
    return E_NOTIMPL;
}

HRESULT CGoProCamFilter::Initialize()
{
    // Create the output pin
    m_pOutputPin = new (std::nothrow) CGoProCamOutputPin(this);
    if (!m_pOutputPin) {
        return E_OUTOFMEMORY;
    }
    return S_OK;
}

BYTE* CGoProCamFilter::GenerateColorBars(int width, int height, int stride)
{
    if (m_pFrameBuffer && (width != m_FrameWidth || height != m_FrameHeight)) {
        CoTaskMemFree(m_pFrameBuffer);
        m_pFrameBuffer = nullptr;
    }
    
    if (!m_pFrameBuffer) {
        m_FrameWidth = width;
        m_FrameHeight = height;
        m_FrameStride = stride;
        m_pFrameBuffer = static_cast<BYTE*>(CoTaskMemAlloc(width * height * 4));
    }
    
    // Generate SMPTE color bars
    int barWidth = width / 8;
    static const struct { BYTE r, g, b; } colors[8] = {
        { 255, 255, 255 }, // White
        { 255, 255, 0 },   // Yellow
        { 0, 255, 255 },   // Cyan
        { 0, 255, 0 },     // Green
        { 255, 0, 255 },   // Magenta
        { 255, 0, 0 },     // Red
        { 0, 0, 255 },     // Blue
        { 0, 0, 0 }        // Black
    };
    
    for (int y = 0; y < height; y++) {
        for (int x = 0; x < width; x++) {
            int bar = x / barWidth;
            if (bar > 7) bar = 7;
            
            BYTE* p = m_pFrameBuffer + (y * stride) + (x * 4);
            p[0] = colors[bar].b;
            p[1] = colors[bar].g;
            p[2] = colors[bar].r;
            p[3] = 255;
        }
    }
    
    return m_pFrameBuffer;
}

// IAMStreamConfig Implementation
IFACEMETHODIMP CGoProCamFilter::SetFormat(AM_MEDIA_TYPE *pmt)
{
    if (!pmt) return E_POINTER;
    
    if (m_pOutputPin) {
        return m_pOutputPin->SetMediaType(pmt);
    }
    return E_FAIL;
}

IFACEMETHODIMP CGoProCamFilter::GetFormat(AM_MEDIA_TYPE **ppmt)
{
    if (!ppmt) return E_POINTER;
    
    if (m_pOutputPin) {
        return m_pOutputPin->ConnectionMediaType(*ppmt);
    }
    return E_FAIL;
}

IFACEMETHODIMP CGoProCamFilter::GetNumberOfCapabilities(int *piCount, int *piSize)
{
    if (!piCount || !piSize) return E_POINTER;
    
    *piCount = 1;
    *piSize = sizeof(VIDEO_STREAM_CONFIG_CAPS);
    return S_OK;
}

IFACEMETHODIMP CGoProCamFilter::GetStreamCaps(int iIndex, AM_MEDIA_TYPE **ppmt, BYTE *pSPC)
{
    if (iIndex != 0) return S_FALSE;
    if (!ppmt || !pSPC) return E_POINTER;
    
    // Create the media type
    VIDEOINFOHEADER* pvi = reinterpret_cast<VIDEOINFOHEADER*>(
        CoTaskMemAlloc(sizeof(VIDEOINFOHEADER)));
    if (!pvi) return E_OUTOFMEMORY;
    
    ZeroMemory(pvi, sizeof(VIDEOINFOHEADER));
    
    // 1080p
    pvi->bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    pvi->bmiHeader.biWidth = 1920;
    pvi->bmiHeader.biHeight = -1080;
    pvi->bmiHeader.biPlanes = 1;
    pvi->bmiHeader.biBitCount = 32;
    pvi->bmiHeader.biCompression = BI_RGB;
    pvi->AvgTimePerFrame = 333333;
    SetRectEmpty(&pvi->rcSource);
    SetRectEmpty(&pvi->rcTarget);
    
    AM_MEDIA_TYPE* pmt = reinterpret_cast<AM_MEDIA_TYPE*>(CoTaskMemAlloc(sizeof(AM_MEDIA_TYPE)));
    if (!pmt) {
        CoTaskMemFree(pvi);
        return E_OUTOFMEMORY;
    }
    
    pmt->majortype = MEDIATYPE_Video;
    pmt->subtype = MEDIASUBTYPE_RGB32;
    pmt->formattype = FORMAT_VideoInfo;
    pmt->bFixedSizeSamples = TRUE;
    pmt->lSampleSize = 1920 * 1080 * 4;
    pmt->cbFormat = sizeof(VIDEOINFOHEADER);
    pmt->pbFormat = reinterpret_cast<BYTE*>(pvi);
    
    *ppmt = pmt;
    
    // Fill stream config caps
    VIDEO_STREAM_CONFIG_CAPABILITIES* pCaps = reinterpret_cast<VIDEO_STREAM_CONFIG_CAPABILITIES*>(pSPC);
    pCaps->guid = FORMAT_VideoInfo;
    pCaps->VideoStandard = AnalogVideo_None;
    pCaps->InputSize.cx = 1920;
    pCaps->InputSize.cy = 1080;
    pCaps->MinCroppingSize.cx = 1920;
    pCaps->MinCroppingSize.cy = 1080;
    pCaps->MaxCroppingSize.cx = 1920;
    pCaps->MaxCroppingSize.cy = 1080;
    pCaps->CropGranularityX = 1;
    pCaps->CropGranularityY = 1;
    pCaps->CropAlignX = 1;
    pCaps->CropAlignY = 1;
    pCaps->MinOutputSize.cx = 1920;
    pCaps->MinOutputSize.cy = 1080;
    pCaps->MaxOutputSize.cx = 1920;
    pCaps->MaxOutputSize.cy = 1080;
    pCaps->OutputGranularityX = 1;
    pCaps->OutputGranularityY = 1;
    pCaps->StretchTapsX = 0;
    pCaps->StretchTapsY = 0;
    pCaps->ShrinkTapsX = 0;
    pCaps->ShrinkTapsY = 0;
    pCaps->MaxFrameInterval = 333333;
    pCaps->MinFrameInterval = 333333;
    pCaps->MinBitsPerSecond = 1920 * 1080 * 4 * 30;
    pCaps->MaxBitsPerSecond = 1920 * 1080 * 4 * 30;
    
    return S_OK;
}

// ============================================================================
// COM Registration
// ============================================================================

static HRESULT SetRegistryValue(HKEY hkeyBase, PCWSTR pszSubKey, PCWSTR pszValueName, PCWSTR pszData)
{
    HKEY hkey;
    HRESULT hr = HRESULT_FROM_WIN32(RegCreateKeyExW(hkeyBase, pszSubKey, 0, nullptr, 
        REG_OPTION_NON_VOLATILE, KEY_WRITE, nullptr, &hkey, nullptr));
    if (FAILED(hr)) return hr;
    
    hr = HRESULT_FROM_WIN32(RegSetValueExW(hkey, pszValueName, 0, REG_SZ,
        reinterpret_cast<const BYTE*>(pszData), 
        (DWORD)((wcslen(pszData) + 1) * sizeof(WCHAR))));
    
    RegCloseKey(hkey);
    return hr;
}

 HRESULT RegisterFilter()
{
    WCHAR szModulePath[MAX_PATH];
    if (!GetModuleFileNameW(g_hModule, szModulePath, MAX_PATH)) {
        return HRESULT_FROM_WIN32(GetLastError());
    }
    
    // CLSID registration
    HRESULT hr = SetRegistryValue(HKEY_CLASSES_ROOT, 
        L"CLSID\\{B7A0C123-8D4E-4F3A-9B5E-6C8D7E2F1A0B}", 
        nullptr, L"GoPro Webcam Studio Filter");
    if (FAILED(hr)) return hr;
    
    // InprocServer32
    hr = SetRegistryValue(HKEY_CLASSES_ROOT,
        L"CLSID\\{B7A0C123-8D4E-4F3A-9B5E-6C8D7E2F1A0B}\\InprocServer32",
        nullptr, szModulePath);
    if (FAILED(hr)) return hr;
    
    hr = SetRegistryValue(HKEY_CLASSES_ROOT,
        L"CLSID\\{B7A0C123-8D4E-4F3A-9B5E-6C8D7E2F1A0B}\\InprocServer32",
        L"ThreadingModel", L"Free");
    if (FAILED(hr)) return hr;
    
    // Alternative name
    hr = SetRegistryValue(HKEY_CLASSES_ROOT,
        L"CLSID\\{B7A0C123-8D4E-4F3A-9B5E-6C8D7E2F1A0B}",
        L"FriendlyName", L"GoPro Webcam Studio");
    if (FAILED(hr)) return hr;
    
    DEBUG_LOG("Filter registered: %ls", szModulePath);
    return S_OK;
}

 HRESULT UnregisterFilter()
{
    RegDeleteTreeW(HKEY_CLASSES_ROOT, L"CLSID\\{B7A0C123-8D4E-4F3A-9B5E-6C8D7E2F1A0B}");
    DEBUG_LOG("Filter unregistered");
    return S_OK;
}