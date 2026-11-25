// Owned DirectX exclusive-fullscreen fixture. Never attaches to League.
#include <windows.h>
#include <d3d11.h>
#include <dxgi.h>
#include <wrl/client.h>
#include <fstream>
#include <iostream>
#include <vector>
#include <thread>
#include <mutex>
#include <string>
#include <atomic>
using Microsoft::WRL::ComPtr;
std::atomic<bool> stopping=false;std::mutex pendingMutex;std::string pending;
LRESULT CALLBACK wnd(HWND h,UINT m,WPARAM w,LPARAM l){if(m==WM_CLOSE){stopping=true;return 0;}return DefWindowProcW(h,m,w,l);}
std::vector<unsigned char> image(std::wstring const& path,int width,int height){
    std::ifstream file(path,std::ios::binary);std::vector<unsigned char> bytes((std::istreambuf_iterator<char>(file)),{});
    if(bytes.size()<54||bytes[0]!='B'||bytes[1]!='M')throw std::runtime_error("Expected owned BMP");
    auto offset=*reinterpret_cast<unsigned*>(&bytes[10]);int w=*reinterpret_cast<int*>(&bytes[18]),h=*reinterpret_cast<int*>(&bytes[22]);int bits=*reinterpret_cast<short*>(&bytes[28]);
    if(w<1||h<1||(bits!=24&&bits!=32)||w>4000||h>2400)throw std::runtime_error("Invalid BMP");int stride=((w*bits+31)/32)*4;
    if(offset+stride*h>bytes.size())throw std::runtime_error("Incomplete BMP");
    std::vector<unsigned char> data(width*height*4);
    for(int y=0;y<height;y++)for(int x=0;x<width;x++){auto source=offset+(h-1-y*h/height)*stride+(x*w/width)*(bits/8);auto dest=(y*width+x)*4;for(int c=0;c<3;c++)data[dest+c]=bytes[source+c];data[dest+3]=255;}
    return data;
}
int wmain(int argc,wchar_t** argv){
    if(argc!=2)return 1;SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    try{
        int width=GetSystemMetrics(SM_CXSCREEN),height=GetSystemMetrics(SM_CYSCREEN);auto instance=GetModuleHandleW(nullptr);
        WNDCLASSW c{};c.lpfnWndProc=wnd;c.hInstance=instance;c.lpszClassName=L"OracleExclusiveFixture";RegisterClassW(&c);
        auto window=CreateWindowW(c.lpszClassName,L"Oracle fullscreen overlay test",WS_POPUP,0,0,width,height,nullptr,nullptr,instance,nullptr);
        DXGI_SWAP_CHAIN_DESC desc{};desc.BufferDesc.Width=width;desc.BufferDesc.Height=height;desc.BufferDesc.Format=DXGI_FORMAT_B8G8R8A8_UNORM;desc.SampleDesc.Count=1;desc.BufferUsage=DXGI_USAGE_RENDER_TARGET_OUTPUT;desc.BufferCount=2;desc.OutputWindow=window;desc.Windowed=TRUE;desc.SwapEffect=DXGI_SWAP_EFFECT_DISCARD;desc.Flags=DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH;
        ComPtr<ID3D11Device> device;ComPtr<ID3D11DeviceContext> context;ComPtr<IDXGISwapChain> swap;
        if(FAILED(D3D11CreateDeviceAndSwapChain(nullptr,D3D_DRIVER_TYPE_HARDWARE,nullptr,D3D11_CREATE_DEVICE_BGRA_SUPPORT,nullptr,0,D3D11_SDK_VERSION,&desc,&swap,&device,nullptr,&context)))throw std::runtime_error("D3D fixture failed");
        ShowWindow(window,SW_SHOW);SetWindowPos(window,HWND_TOP,0,0,width,height,SWP_SHOWWINDOW);SetForegroundWindow(window);
        auto previous=GetForegroundWindow();auto other=GetWindowThreadProcessId(previous,nullptr);auto own=GetCurrentThreadId();
        if(previous!=window&&other!=own&&AttachThreadInput(own,other,TRUE)){SetForegroundWindow(window);AttachThreadInput(own,other,FALSE);}
        for(int step=0;step<20;step++){MSG message{};while(PeekMessageW(&message,nullptr,0,0,PM_REMOVE)){TranslateMessage(&message);DispatchMessageW(&message);}Sleep(10);}
        SetActiveWindow(window);auto foreground=GetForegroundWindow();auto targetThread=GetWindowThreadProcessId(foreground,nullptr);
        BOOL attached=AttachThreadInput(GetCurrentThreadId(),targetThread,TRUE);SetForegroundWindow(window);if(attached)AttachThreadInput(GetCurrentThreadId(),targetThread,FALSE);
        Sleep(100);
        auto fullscreenResult=swap->SetFullscreenState(TRUE,nullptr);
        if(FAILED(fullscreenResult)){std::cerr<<"Fullscreen HRESULT: "<<std::hex<<fullscreenResult<<" visible="<<IsWindowVisible(window)<<" front="<<(GetForegroundWindow()==window)<<std::endl;throw std::runtime_error("Exclusive fullscreen unavailable");}
        ComPtr<ID3D11Texture2D> back;swap->GetBuffer(0,IID_PPV_ARGS(&back));D3D11_TEXTURE2D_DESC textureDesc{};back->GetDesc(&textureDesc);textureDesc.BindFlags=0;textureDesc.Usage=D3D11_USAGE_DEFAULT;
        auto makeTexture=[&](std::wstring const& path){auto pixels=image(path,width,height);D3D11_SUBRESOURCE_DATA initial{pixels.data(),static_cast<UINT>(width*4),0};ComPtr<ID3D11Texture2D> texture;if(FAILED(device->CreateTexture2D(&textureDesc,&initial,&texture)))throw std::runtime_error("Fixture image failed");return texture;};
        auto texture=makeTexture(argv[1]);BOOL fullscreen=FALSE;swap->GetFullscreenState(&fullscreen,nullptr);
        std::cout<<"{\"ready\":true,\"exclusive\":"<<(fullscreen?"true":"false")<<",\"hwnd\":"<<reinterpret_cast<uintptr_t>(window)<<",\"width\":"<<width<<",\"height\":"<<height<<"}"<<std::endl;
        std::thread([]{std::string line;while(std::getline(std::cin,line)){if(line=="quit"){stopping=true;return;}std::lock_guard<std::mutex> guard(pendingMutex);pending=line;}stopping=true;}).detach();
        MSG msg{};while(!stopping){while(PeekMessageW(&msg,nullptr,0,0,PM_REMOVE)){TranslateMessage(&msg);DispatchMessageW(&msg);}std::string path;{std::lock_guard<std::mutex> guard(pendingMutex);path.swap(pending);}if(!path.empty()){texture=makeTexture(std::wstring(path.begin(),path.end()));std::cout<<"updated"<<std::endl;}context->CopyResource(back.Get(),texture.Get());if(FAILED(swap->Present(1,0)))break;}
        swap->SetFullscreenState(FALSE,nullptr);DestroyWindow(window);return 0;
    }catch(std::exception const& e){std::cerr<<e.what()<<std::endl;return 1;}
}
