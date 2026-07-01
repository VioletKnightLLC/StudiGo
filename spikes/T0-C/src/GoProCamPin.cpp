// GoProCamPin.cpp - Output Pin Implementation for GoPro Virtual Camera Filter

#include "GoProCamFilter.h"
#include <strsafe.h>

#ifdef _DEBUG
#define DEBUG_LOG(x, ...) fprintf(stderr, "[GoProCam] " x "\n", __VA_ARGS__)
#else
#define DEBUG_LOG(x, ...) 
#endif

// ============================================================================
// CGoProCamOutputPin Implementation
// ============================================================================

CGoProCamOutputPin::CGoProCamOutputPin(CGoProCamFilter *pFilter)
    : m_pFilter(pFilter)
{
    // Set default media type
    ZeroMemory(&m_mt, sizeof(m_mt));
    
    // Create default VIDEOINFOHEADER
    VIDEOINFOHEADER* pvi = reinterpret_cast<VIDEOINFOHEADER*>(
        CoTaskMemAlloc(sizeof(VIDEOINFOHEADER)));
    ZeroMemory(pvi, sizeof(VIDEOINFOHEADER));
    
    // Set dimensions (1920x1080 BGRA)
    pvi->bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    pvi->bmiHeader.biWidth = 1920;
    pvi->bmiHeader.biHeight = -1080;  // Top-down
    pvi->bmiHeader.biPlanes = 1;
    pvi->bmiHeader.biBitCount = 32;
    pvi->bmiHeader.biCompression = BI_RGB;
    pvi->bmiHeader.biSizeImage = 1920 * 1080 * 4;
    
    // Timing
    pvi->AvgTimePerFrame = 333333; // ~30fps
    
    // Source rect
    SetRectEmpty(&pvi->rcSource);
    SetRectEmpty(&pvi->rcTarget);
    
    m_mt.pbFormat = reinterpret_cast<BYTE*>(pvi);
    m_mt.cbFormat = sizeof(VIDEOINFOHEADER);
    m_mt.majortype = MEDIATYPE_Video;
    m_mt.subtype = MEDIASUBTYPE_BGRA32;
    m_mt.formattype = FORMAT_VideoInfo;
    m_mt.bFixedSizeSamples = TRUE;
    m_mt.lSampleSize = 1920 * 1080 * 4;
    
    StringCchCopyW(m_Id, MAX_PIN_NAME, L"Output");
    
    DEBUG_LOG("Output pin created");
}

CGoProCamOutputPin::~CGoProCamOutputPin()
{
    if (m_mt.pbFormat) {
        CoTaskMemFree(m_mt.pbFormat);
    }
    DEBUG_LOG("Output pin destroyed");
}

// IUnknown
IFACEMETHODIMP CGoProCamOutputPin::QueryInterface(REFIID riid, void **ppv)
{
    if (!ppv) return E_POINTER;
    
    if (riid == IID_IUnknown || riid == IID_IPin) {
        *ppv = static_cast<IPin*>(this);
        AddRef();
        return S_OK;
    }
    
    *ppv = nullptr;
    return E_NOINTERFACE;
}

IFACEMETHODIMP_(ULONG) CGoProCamOutputPin::AddRef()
{
    return InterlockedIncrement(&m_cRef);
}

IFACEMETHODIMP_(ULONG) CGoProCamOutputPin::Release()
{
    LONG cRef = InterlockedDecrement(&m_cRef);
    if (cRef == 0) {
        delete this;
    }
    return cRef;
}

// IPin Implementation
IFACEMETHODIMP CGoProCamOutputPin::Connect(IPin *pReceivePin, const AM_MEDIA_TYPE *pmt)
{
    if (!pReceivePin) return E_POINTER;
    return E_NOTIMPL;
}

IFACEMETHODIMP CGoProCamOutputPin::ReceiveConnection(IPin *pConnector, const AM_MEDIA_TYPE *pmt)
{
    if (!pConnector || !pmt) return E_POINTER;
    
    if (pmt->majortype != MEDIATYPE_Video) {
        return VFW_E_TYPE_NOT_ACCEPTED;
    }
    
    m_pConnectedPin = pConnector;
    
    return SetMediaType(pmt);
}

IFACEMETHODIMP CGoProCamOutputPin::Disconnect()
{
    m_pConnectedPin = nullptr;
    return S_OK;
}

IFACEMETHODIMP CGoProCamOutputPin::ConnectedTo(IPin **pPin)
{
    if (!pPin) return E_POINTER;
    *pPin = m_pConnectedPin;
    if (*pPin) {
        (*pPin)->AddRef();
    }
    return S_OK;
}

IFACEMETHODIMP CGoProCamOutputPin::ConnectionMediaType(AM_MEDIA_TYPE *pmt)
{
    if (!pmt) return E_POINTER;
    CopyMemory(pmt, &m_mt, sizeof(AM_MEDIA_TYPE));
    return S_OK;
}

IFACEMETHODIMP CGoProCamOutputPin::QueryPinInfo(PIN_INFO *pInfo)
{
    if (!pInfo) return E_POINTER;
    
    pInfo->pFilter = static_cast<IBaseFilter*>(m_pFilter);
    pInfo->pFilter->AddRef();
    pInfo->dir = m_PinDir;
    StringCchCopyW(pInfo->Name, MAX_PIN_NAME, m_Id);
    
    return S_OK;
}

IFACEMETHODIMP CGoProCamOutputPin::QueryDirection(PIN_DIRECTION *pPinDir)
{
    if (!pPinDir) return E_POINTER;
    *pPinDir = m_PinDir;
    return S_OK;
}

IFACEMETHODIMP CGoProCamOutputPin::QueryId(LPWSTR *lpwstrId)
{
    if (!lpwstrId) return E_POINTER;
    
    *lpwstrId = static_cast<LPWSTR>(CoTaskMemAlloc(MAX_PIN_NAME * sizeof(WCHAR)));
    if (!*lpwstrId) return E_OUTOFMEMORY;
    
    StringCchCopyW(*lpwstrId, MAX_PIN_NAME, m_Id);
    return S_OK;
}

IFACEMETHODIMP CGoProCamOutputPin::QueryAccept(const AM_MEDIA_TYPE *pmt)
{
    if (!pmt) return E_POINTER;
    return CheckMediaType(pmt) == S_OK ? S_OK : S_FALSE;
}

// Media type enumerator
class CEnumMediaTypes : public IEnumMediaTypes
{
public:
    CEnumMediaTypes() : m_cRef(1), m_position(0) {}
    
    // IUnknown
    IFACEMETHODIMP QueryInterface(REFIID riid, void **ppv) {
        if (riid == IID_IUnknown || riid == IID_IEnumMediaTypes) {
            *ppv = static_cast<IEnumMediaTypes*>(this);
            AddRef();
            return S_OK;
        }
        *ppv = nullptr;
        return E_NOINTERFACE;
    }
    IFACEMETHODIMP_(ULONG) AddRef() { return InterlockedIncrement(&m_cRef); }
    IFACEMETHODIMP_(ULONG) Release() { 
        LONG cRef = InterlockedDecrement(&m_cRef);
        if (cRef == 0) delete this;
        return cRef;
    }
    
    // IEnumMediaTypes
    IFACEMETHODIMP Next(ULONG cMediaTypes, AM_MEDIA_TYPE **ppMediaTypes, ULONG *pcFetched) {
        if (!ppMediaTypes) return E_POINTER;
        if (m_position >= 1) return S_FALSE;
        
        // Only support this one type
        VIDEOINFOHEADER* pvi = reinterpret_cast<VIDEOINFOHEADER*>(
            CoTaskMemAlloc(sizeof(VIDEOINFOHEADER)));
        ZeroMemory(pvi, sizeof(VIDEOINFOHEADER));
        
        pvi->bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
        pvi->bmiHeader.biWidth = 1920;
        pvi->bmiHeader.biHeight = -1080;
        pvi->bmiHeader.biPlanes = 1;
        pvi->bmiHeader.biBitCount = 32;
        pvi->bmiHeader.biCompression = BI_RGB;
        pvi->bmiHeader.biSizeImage = 1920 * 1080 * 4;
        pvi->AvgTimePerFrame = 333333;
        SetRectEmpty(&pvi->rcSource);
        SetRectEmpty(&pvi->rcTarget);
        
        AM_MEDIA_TYPE* pmt = reinterpret_cast<AM_MEDIA_TYPE*>(CoTaskMemAlloc(sizeof(AM_MEDIA_TYPE)));
        pmt->majortype = MEDIATYPE_Video;
        pmt->subtype = MEDIASUBTYPE_BGRA32;
        pmt->formattype = FORMAT_VideoInfo;
        pmt->bFixedSizeSamples = TRUE;
        pmt->lSampleSize = 1920 * 1080 * 4;
        pmt->cbFormat = sizeof(VIDEOINFOHEADER);
        pmt->pbFormat = reinterpret_cast<BYTE*>(pvi);
        
        ppMediaTypes[0] = pmt;
        m_position++;
        if (pcFetched) *pcFetched = 1;
        return S_OK;
    }
    IFACEMETHODIMP Skip(ULONG cMediaTypes) {
        m_position += cMediaTypes;
        return m_position <= 1 ? S_OK : S_FALSE;
    }
    IFACEMETHODIMP Reset() { m_position = 0; return S_OK; }
    IFACEMETHODIMP Clone(IEnumMediaTypes **ppEnum) { return E_NOTIMPL; }
    
private:
    LONG m_cRef;
    int m_position;
};

IFACEMETHODIMP CGoProCamOutputPin::EnumMediaTypes(IEnumMediaTypes **ppEnum)
{
    if (!ppEnum) return E_POINTER;
    
    *ppEnum = new (std::nothrow) CEnumMediaTypes();
    return *ppEnum ? S_OK : E_OUTOFMEMORY;
}

IFACEMETHODIMP CGoProCamOutputPin::EndOfStream()
{
    return S_OK;
}

IFACEMETHODIMP CGoProCamOutputPin::BeginFlush()
{
    return S_OK;
}

IFACEMETHODIMP CGoProCamOutputPin::EndFlush()
{
    return S_OK;
}

IFACEMETHODIMP CGoProCamOutputPin::NewSegment(REFERENCE_TIME tStart, REFERENCE_TIME tStop, double dRate)
{
    return S_OK;
}

IFACEMETHODIMP CGoProCamOutputPin::QueryInternalConnections(IPin **apPin, ULONG *pcPins)
{
    return E_NOTIMPL;
}

// Helper methods
HRESULT CGoProCamOutputPin::CheckMediaType(const AM_MEDIA_TYPE *pmt)
{
    if (!pmt) return E_POINTER;
    
    if (pmt->majortype != MEDIATYPE_Video) {
        return E_FAIL;
    }
    
    return S_OK;
}

HRESULT CGoProCamOutputPin::SetMediaType(const AM_MEDIA_TYPE *pmt)
{
    if (!pmt) return E_POINTER;
    
    if (m_mt.pbFormat) {
        CoTaskMemFree(m_mt.pbFormat);
    }
    
    CopyMemory(&m_mt, pmt, sizeof(AM_MEDIA_TYPE));
    
    if (pmt->cbFormat && pmt->pbFormat) {
        m_mt.pbFormat = CoTaskMemAlloc(pmt->cbFormat);
        if (!m_mt.pbFormat) {
            return E_OUTOFMEMORY;
        }
        CopyMemory(m_mt.pbFormat, pmt->pbFormat, pmt->cbFormat);
    }
    
    return S_OK;
}