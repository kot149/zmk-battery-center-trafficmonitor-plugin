#include "PluginInterface.h"

#include <array>
#include <cstddef>
#include <string>

extern "C" bool zmk_battery_refresh(
    wchar_t* value,
    std::size_t value_capacity,
    wchar_t* tooltip,
    std::size_t tooltip_capacity);

static_assert(sizeof(wchar_t) == 2, "TrafficMonitor requires Windows UTF-16 wchar_t");

class BatteryItem final : public IPluginItem
{
public:
    const wchar_t* GetItemName() const override { return L"ZMK Battery"; }
    const wchar_t* GetItemId() const override { return L"zmk_battery"; }
    const wchar_t* GetItemLableText() const override { return L"ZMK"; }
    const wchar_t* GetItemValueText() const override { return value_.c_str(); }
    const wchar_t* GetItemValueSampleText() const override { return L"Central 100% | Peripheral 100%"; }

    void Refresh()
    {
        std::array<wchar_t, 2048> value{};
        std::array<wchar_t, 8192> tooltip{};
        if (zmk_battery_refresh(value.data(), value.size(), tooltip.data(), tooltip.size()))
        {
            value_ = value.data();
            tooltip_ = tooltip.data();
        }
    }

    const wchar_t* Tooltip() const { return tooltip_.c_str(); }

private:
    std::wstring value_{L"N/A"};
    std::wstring tooltip_{L"Waiting for zmk-battery-center data"};
};

class BatteryPlugin final : public ITMPlugin
{
public:
    IPluginItem* GetItem(int index) override
    {
        return index == 0 ? &item_ : nullptr;
    }

    void DataRequired() override
    {
        item_.Refresh();
    }

    const wchar_t* GetInfo(PluginInfoIndex index) override
    {
        switch (index)
        {
        case TMI_NAME:
            return L"ZMK Battery Center";
        case TMI_DESCRIPTION:
            return L"Displays battery snapshots published by zmk-battery-center.";
        case TMI_AUTHOR:
            return L"kot149";
        case TMI_COPYRIGHT:
            return L"Copyright (c) 2026 kot149";
        case TMI_VERSION:
            return L"0.1.0";
        case TMI_URL:
            return L"https://github.com/kot149/zmk-battery-center";
        default:
            return L"";
        }
    }

    const wchar_t* GetTooltipInfo() override
    {
        return item_.Tooltip();
    }

private:
    BatteryItem item_;
};

static BatteryPlugin plugin;

extern "C" __declspec(dllexport) ITMPlugin* TMPluginGetInstance()
{
    return &plugin;
}

extern "C" void zmk_tm_link_anchor() {}
