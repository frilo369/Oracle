// Oracle's separate UWP view, hosted by the Xbox Game Bar compositor.
// Only a bounded, expiring local label document crosses from the desktop app.
#include <windows.h>
#undef GetCurrentTime
#include <winrt/Windows.ApplicationModel.Activation.h>
#include <winrt/Windows.Data.Json.h>
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.Graphics.Display.h>
#include <winrt/Windows.UI.Core.h>
#include <winrt/Windows.Storage.h>
#include <winrt/Windows.Storage.FileProperties.h>
#include <winrt/Windows.UI.Xaml.h>
#include <winrt/Windows.UI.Xaml.Controls.h>
#include <winrt/Windows.UI.Xaml.Interop.h>
#include <winrt/Windows.UI.Xaml.Media.h>
#include <winrt/Windows.UI.Xaml.Media.Imaging.h>
#include <winrt/Windows.Graphics.Imaging.h>
#include <winrt/Windows.Storage.Streams.h>
#include <winrt/Microsoft.Gaming.XboxGameBar.h>
#include <algorithm>
#include <array>
#include <chrono>
#include <memory>
#include <fstream>

using namespace winrt;
using namespace Windows::ApplicationModel::Activation;
using namespace Windows::Data::Json;
using namespace Windows::Foundation;
using namespace Windows::Storage;
using namespace Windows::UI;
using namespace Windows::UI::Xaml;
using namespace Windows::UI::Xaml::Controls;
using namespace Windows::UI::Xaml::Media;
using namespace Microsoft::Gaming::XboxGameBar;

static int64_t now_ms() {return std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::system_clock::now().time_since_epoch()).count();}
static void diagnostic(std::string const& message){try{std::ofstream log(ApplicationData::Current().LocalFolder().Path().c_str()+std::wstring(L"/widget-lifecycle.txt"),std::ios::app);log<<message<<"\n";}catch(...){}}
static SolidColorBrush brush(uint8_t a,uint8_t r,uint8_t g,uint8_t b){return SolidColorBrush(Color{a,r,g,b});}

struct View : std::enable_shared_from_this<View> {
    XboxGameBarWidget host{nullptr};
    Frame frame{nullptr};Canvas canvas{nullptr};TextBlock help{nullptr};
    std::array<Border,3> cards{nullptr,nullptr,nullptr};
    std::array<TextBlock,3> texts{nullptr,nullptr,nullptr};
    DispatcherTimer timer{nullptr};bool reading=false;bool stopped=false;bool snapshotSaved=false;
    std::wstring last;Size requested{};int rendered=0;int64_t documentAt=0;
    void initialize(XboxGameBarWidgetActivatedEventArgs const& args) {
        frame=Frame();Window::Current().Content(frame);
        host=XboxGameBarWidget(args,Window::Current().CoreWindow(),frame);
        // The SDK observes Frame navigation when attaching the widget visual.
        // Follow its bootstrap order: initialize host, then navigate to a Page.
        if(!frame.Navigate(xaml_typename<Page>()))throw hresult_error(E_FAIL,L"Oracle widget page navigation failed");
        auto page=frame.Content().as<Page>();canvas=Canvas();page.Background(brush(0,0,0,0));page.Content(canvas);
        host.MinWindowSize({400,100});host.MaxWindowSize({7680,4320});
        canvas.Background(brush(0,0,0,0));canvas.IsHitTestVisible(false);
        help=TextBlock();help.TextWrapping(TextWrapping::Wrap);help.FontSize(17);help.Foreground(brush(255,244,216,175));help.Width(660);
        Canvas::SetLeft(help,24);Canvas::SetTop(help,35);canvas.Children().Append(help);
        for(size_t i=0;i<3;i++) {
            texts[i]=TextBlock();texts[i].FontSize(15);texts[i].Foreground(brush(255,244,216,175));texts[i].HorizontalAlignment(HorizontalAlignment::Center);texts[i].VerticalAlignment(VerticalAlignment::Center);
            cards[i]=Border();cards[i].Background(brush(245,16,18,27));cards[i].BorderBrush(brush(255,190,154,110));cards[i].BorderThickness({1,1,1,1});cards[i].Height(34);cards[i].Child(texts[i]);cards[i].Visibility(Visibility::Collapsed);canvas.Children().Append(cards[i]);
        }
        auto weak=weak_from_this();
        Window::Current().Closed([weak](auto&&,auto&&){if(auto self=weak.lock()){self->stopped=true;self->timer.Stop();}});
        timer=DispatcherTimer();timer.Interval(std::chrono::milliseconds(150));
        timer.Tick([weak](auto&&,auto&&){if(auto self=weak.lock())self->read();});timer.Start();
        Window::Current().Activate();read();
    }
    void hide() {rendered=0;snapshotSaved=false;for(auto const& card:cards)card.Visibility(Visibility::Collapsed);}
    fire_and_forget snapshot() {
        auto self=shared_from_this();
        try {
            Windows::UI::Xaml::Media::Imaging::RenderTargetBitmap bitmap;
            co_await bitmap.RenderAsync(frame);
            auto buffer=co_await bitmap.GetPixelsAsync();com_array<uint8_t> bytes(buffer.Length());
            Windows::Storage::Streams::DataReader::FromBuffer(buffer).ReadBytes(bytes);
            auto file=co_await ApplicationData::Current().LocalFolder().CreateFileAsync(L"widget-preview.png",CreationCollisionOption::ReplaceExisting);
            auto stream=co_await file.OpenAsync(FileAccessMode::ReadWrite);
            auto encoder=co_await Windows::Graphics::Imaging::BitmapEncoder::CreateAsync(Windows::Graphics::Imaging::BitmapEncoder::PngEncoderId(),stream);
            encoder.SetPixelData(Windows::Graphics::Imaging::BitmapPixelFormat::Bgra8,Windows::Graphics::Imaging::BitmapAlphaMode::Premultiplied,bitmap.PixelWidth(),bitmap.PixelHeight(),96,96,bytes);
            co_await encoder.FlushAsync();
        }catch(hresult_error const& e){diagnostic("Owned widget snapshot failed: "+to_string(e.message()));}
    }
    void guidance(hstring const& message) {
        help.Text(message);
        help.Visibility(host.GameBarDisplayMode()==XboxGameBarDisplayMode::Foreground?Visibility::Visible:Visibility::Collapsed);
    }
    fire_and_forget resize(double width,double height,double dpi) {
        auto self=shared_from_this();
        try {
            Size wanted{static_cast<float>(std::clamp(width*.96/dpi,400.,7680.)),static_cast<float>(std::clamp(height*.88/dpi,100.,4320.))};
            if(std::abs(wanted.Width-requested.Width)<2&&std::abs(wanted.Height-requested.Height)<2)co_return;
            requested=wanted;if(!co_await host.TryResizeWindowAsync(wanted))requested={};co_await host.CenterWindowAsync();
        }catch(...){requested={};}
    }
    fire_and_forget read() {
        if(reading||stopped)co_return;
        auto self=shared_from_this();reading=true;
        try {
            auto folder=ApplicationData::Current().LocalFolder();
            // Heartbeat is a capability-free acknowledgement. No game text or pixels.
            auto bounds=host.WindowBounds();
            JsonObject status;status.SetNamedValue(L"schema",JsonValue::CreateNumberValue(1));status.SetNamedValue(L"at",JsonValue::CreateNumberValue(static_cast<double>(now_ms())));
            status.SetNamedValue(L"pinned",JsonValue::CreateBooleanValue(host.Pinned()));status.SetNamedValue(L"clickThrough",JsonValue::CreateBooleanValue(host.ClickThroughEnabled()));status.SetNamedValue(L"visible",JsonValue::CreateBooleanValue(host.Visible()));
            status.SetNamedValue(L"rendered",JsonValue::CreateNumberValue(rendered));status.SetNamedValue(L"documentAt",JsonValue::CreateNumberValue(static_cast<double>(documentAt)));
            status.SetNamedValue(L"opacity",JsonValue::CreateNumberValue(host.RequestedOpacity()));status.SetNamedValue(L"canvasWidth",JsonValue::CreateNumberValue(canvas.ActualWidth()));status.SetNamedValue(L"canvasHeight",JsonValue::CreateNumberValue(canvas.ActualHeight()));
            status.SetNamedValue(L"coreVisible",JsonValue::CreateBooleanValue(Window::Current().CoreWindow().Visible()));
            status.SetNamedValue(L"displayMode",JsonValue::CreateNumberValue(static_cast<int>(host.GameBarDisplayMode())));
            status.SetNamedValue(L"windowX",JsonValue::CreateNumberValue(bounds.X));status.SetNamedValue(L"windowY",JsonValue::CreateNumberValue(bounds.Y));status.SetNamedValue(L"windowWidth",JsonValue::CreateNumberValue(bounds.Width));status.SetNamedValue(L"windowHeight",JsonValue::CreateNumberValue(bounds.Height));
            {auto temp=folder.Path().c_str()+std::wstring(L"/widget-status.tmp");{std::ofstream out(temp,std::ios::binary|std::ios::trunc);out<<to_string(status.Stringify());}MoveFileExW(temp.c_str(),(folder.Path().c_str()+std::wstring(L"/widget-status.json")).c_str(),MOVEFILE_REPLACE_EXISTING);}
            std::ifstream input(folder.Path().c_str()+std::wstring(L"/overlay.json"),std::ios::binary);
            if(!input){hide();guidance(L"Oracle: pin this widget and enable Game Bar click-through. Then enable the overlay in Oracle Settings.");reading=false;co_return;}
            input.seekg(0,std::ios::end);auto length=input.tellg();if(length<0||length>16384)throw hresult_invalid_argument();input.seekg(0);
            std::string raw(static_cast<size_t>(length),'\0');input.read(raw.data(),length);input.close();JsonObject doc=JsonObject::Parse(to_hstring(raw));
            auto at=static_cast<int64_t>(doc.GetNamedNumber(L"at",0));auto age=now_ms()-at;
            if(doc.GetNamedNumber(L"schema",0)!=1||age<0||age>1500||!doc.GetNamedBoolean(L"visible",false)) {
                hide();guidance(L"Oracle is waiting for an active Arena or Mayhem augment selection. Pin this widget and enable click-through.");reading=false;co_return;
            }
            if(!host.Pinned()||!host.ClickThroughEnabled()) {
                hide();guidance(L"Pin Oracle and enable Game Bar's click-through button so League keeps all mouse input.");reading=false;co_return;
            }
            auto dpi=Windows::Graphics::Display::DisplayInformation::GetForCurrentView().RawPixelsPerViewPixel();
            double width=doc.GetNamedNumber(L"width",0),height=doc.GetNamedNumber(L"height",0);
            if(width<640||width>10000||height<400||height>10000)throw hresult_invalid_argument();
            resize(width,height,dpi);bounds=host.WindowBounds();
            auto labels=doc.GetNamedArray(L"labels");if(labels.Size()!=3)throw hresult_invalid_argument();
            double originX=doc.GetNamedNumber(L"screenX",0),originY=doc.GetNamedNumber(L"screenY",0);
            rendered=0;documentAt=at;
            for(uint32_t i=0;i<3;i++) {
                auto label=labels.GetObjectAt(i);auto text=label.GetNamedString(L"text");
                double x=label.GetNamedNumber(L"x"),y=label.GetNamedNumber(L"y");
                if(text.size()>100||x<0||x>width||y<0||y>height)throw hresult_invalid_argument();
                double barWidth=std::clamp(width*.19/dpi,150.,420.);
                // The SDK returns XAML opacity (0..1), as in Microsoft's sample.
                // Dividing by 100 leaves correctly positioned labels invisible.
                texts[i].Text(text);cards[i].Width(barWidth);cards[i].Opacity(std::clamp(host.RequestedOpacity(),0.,1.));
                Canvas::SetLeft(cards[i],(originX+x)/dpi-bounds.X-barWidth/2);Canvas::SetTop(cards[i],(originY+y)/dpi-bounds.Y);
                cards[i].Visibility(Visibility::Visible);
                if(Canvas::GetLeft(cards[i])>=0&&Canvas::GetTop(cards[i])>=0&&Canvas::GetLeft(cards[i])+barWidth<=bounds.Width&&Canvas::GetTop(cards[i])+34<=bounds.Height)rendered++;
            }
            help.Visibility(Visibility::Collapsed);
            if(doc.GetNamedBoolean(L"qa",false)&&rendered==3&&!snapshotSaved){snapshotSaved=true;snapshot();}
        }catch(...){hide();guidance(L"Oracle is disconnected or its overlay data expired. Keep the desktop app running.");}
        reading=false;
        co_return;
    }
};

struct OracleApp : ApplicationT<OracleApp> {
    std::shared_ptr<View> view;
    OracleApp(){diagnostic("App constructed");UnhandledException([](auto&&,UnhandledExceptionEventArgs const& e){diagnostic("Unhandled: "+to_string(e.Message()));e.Handled(true);});}
    void OnActivated(IActivatedEventArgs const& args) {
        diagnostic("OnActivated");
        try {
            auto widgetArgs=args.try_as<XboxGameBarWidgetActivatedEventArgs>();
            if(widgetArgs&&widgetArgs.IsLaunchActivation()) {view=std::make_shared<View>();view->initialize(widgetArgs);}
        }catch(hresult_error const& e){diagnostic("Activate failed: "+to_string(e.message()));Application::Current().Exit();}
    }
    void OnLaunched(LaunchActivatedEventArgs const&) {
        diagnostic("OnLaunched");
        TextBlock info;info.Text(L"Open Win+G, choose Oracle Augments, then pin it and enable click-through.");info.Margin({30,30,30,30});info.TextWrapping(TextWrapping::Wrap);
        Window::Current().Content(info);Window::Current().Activate();
    }
};
int WINAPI wWinMain(HINSTANCE,HINSTANCE,PWSTR,int) {
    try {
        init_apartment(apartment_type::multi_threaded);
        com_ptr<OracleApp> application;
        Application::Start([&](auto&&){application=make_self<OracleApp>();});return 0;
    }catch(hresult_error const& error){
        try{std::ofstream log(ApplicationData::Current().LocalFolder().Path().c_str()+std::wstring(L"/startup-error.txt"));log<<std::hex<<error.code().value<<" "<<to_string(error.message());}catch(...){}
        return 1;
    }
}
