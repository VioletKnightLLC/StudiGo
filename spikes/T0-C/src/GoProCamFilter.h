#pragma once

#include <windows.h>
#include <dshow.h>
#include <uuids.h>
#include <initguid.h>
#include <strsafe.h>

// Filter CLSID
// {B7A0C123-8D4E-4F3A-9B5E-6C8D7E2F1A0B}
DEFINE_GUID(CLSID_GoProCamFilter,
    0xb7a0c123, 0x8d4e, 0x4f3a, 0x9b, 0x5e, 0x6c, 0x8d, 0x7e, 0x2f, 0x1a, 0x0b);

// Forward declarations
class CGoProCamOutputPin;

// Main filter class
class CGoProCamFilter : public IBaseFilter, public IAMStreamConfig
{
    friend class CGoProCamOutputPin;

public:
    CGoProCamFilter();
    virtual ~CGoProCamFilter();

    // Called after construction to set up the filter
    HRESULT Init() {
        return Initialize();
    }

    // IUnknown
    IFACEMETHODIMP QueryInterface(REFIID riid, void **ppv);
    IFACEMETHODIMP_(ULONG) AddRef();
    IFACEMETHODIMP_(ULONG) Release();

    // IBaseFilter
    IFACEMETHODIMP GetClassID(CLSID *pClsID);
    IFACEMETHODIMP Stop();
    IFACEMETHODIMP Pause();
    IFACEMETHODIMP Run(REFERENCE_TIME rtStart);
    IFACEMETHODIMP GetState(DWORD dwState, FILTER_STATE *pState);
    IFACEMETHODIMP SetSyncSource(IReferenceClock *pClock);
    IFACEMETHODIMP GetSyncSource(IReferenceClock **ppClock);
    IFACEMETHODIMP EnumPins(IEnumPins **ppEnum);
    IFACEMETHODIMP FindPin(LPCWSTR lpwstrPinId, IPin **ppPin);
    IFACEMETHODIMP QueryFilterInfo(FILTER_INFO *pInfo);
    IFACEMETHODIMP JoinFilterGraph(IFilterGraph *pGraph, LPCWSTR lpwstrName);
    IFACEMETHODIMP QueryVendorInfo(LPWSTR *pVendorInfo);

    // IAMStreamConfig
    IFACEMETHODIMP SetFormat(AM_MEDIA_TYPE *pmt);
    IFACEMETHODIMP GetFormat(AM_MEDIA_TYPE **ppmt);
    IFACEMETHODIMP GetNumberOfCapabilities(int *piCount, int *piSize);
    IFACEMETHODIMP GetStreamCaps(int iIndex, AM_MEDIA_TYPE **ppmt, BYTE *pSPC);

    // Non-interface methods
    HRESULT Initialize();
    CGoProCamOutputPin* GetOutputPin() { return m_pOutputPin; }

    // Frame buffer for test pattern
    BYTE* GetFrameBuffer() { return m_pFrameBuffer; }
    int GetFrameWidth() { return m_FrameWidth; }
    int GetFrameHeight() { return m_FrameHeight; }

private:
    LONG m_cRef = 1;
    FILTER_STATE m_State = State_Stopped;
    IReferenceClock *m_pClock = nullptr;
    IFilterGraph *m_pGraph = nullptr;
    CGoProCamOutputPin* m_pOutputPin = nullptr;
    WCHAR m_Name[MAX_FILTER_NAME] = L"GoPro Webcam Studio";
    
    // Frame buffer
    BYTE* m_pFrameBuffer = nullptr;
    int m_FrameWidth = 1920;
    int m_FrameHeight = 1080;
    
    void GenerateColorBars();
};

// Output pin class
class CGoProCamOutputPin : public IPin
{
    friend class CGoProCamFilter;

public:
    CGoProCamOutputPin(CGoProCamFilter *pFilter);
    virtual ~CGoProCamOutputPin();

    // IUnknown
    IFACEMETHODIMP QueryInterface(REFIID riid, void **ppv);
    IFACEMETHODIMP_(ULONG) AddRef();
    IFACEMETHODIMP_(ULONG) Release();

    // IPin
    IFACEMETHODIMP Connect(IPin *pReceivePin, const AM_MEDIA_TYPE *pmt);
    IFACEMETHODIMP ReceiveConnection(IPin *pConnector, const AM_MEDIA_TYPE *pmt);
    IFACEMETHODIMP Disconnect();
    IFACEMETHODIMP ConnectedTo(IPin **pPin);
    IFACEMETHODIMP ConnectionMediaType(AM_MEDIA_TYPE *pmt);
    IFACEMETHODIMP QueryPinInfo(PIN_INFO *pInfo);
    IFACEMETHODIMP QueryDirection(PIN_DIRECTION *pPinDir);
    IFACEMETHODIMP QueryId(LPWSTR *lpwstrId);
    IFACEMETHODIMP QueryAccept(const AM_MEDIA_TYPE *pmt);
    IFACEMETHODIMP EnumMediaTypes(IEnumMediaTypes **ppEnum);
    IFACEMETHODIMP EndOfStream();
    IFACEMETHODIMP BeginFlush();
    IFACEMETHODIMP EndFlush();
    IFACEMETHODIMP NewSegment(REFERENCE_TIME tStart, REFERENCE_TIME tStop, double dRate);
    IFACEMETHODIMP QueryInternalConnections(IPin **apPin, ULONG *pcPins);

    // Helper methods
    HRESULT CheckMediaType(const AM_MEDIA_TYPE *pmt);
    HRESULT SetMediaType(const AM_MEDIA_TYPE *pmt);

    CGoProCamFilter* GetFilter() { return m_pFilter; }

private:
    LONG m_cRef = 1;
    CGoProCamFilter* m_pFilter = nullptr;
    IPin* m_pConnectedPin = nullptr;
    AM_MEDIA_TYPE m_mt = {};
    PIN_DIRECTION m_PinDir = PINDIR_OUTPUT;
    WCHAR m_Id[MAX_PIN_NAME] = L"Output";
};

// COM registration functions
HRESULT RegisterFilter();
HRESULT UnregisterFilter();

// Global module handle
extern HMODULE g_hModule;