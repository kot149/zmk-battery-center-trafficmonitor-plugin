#include "PluginInterface.h"

#include <array>
#include <cstddef>
#include <string>
#include <utility>
#include <vector>

extern "C" std::size_t zmk_battery_device_count();
extern "C" bool zmk_battery_device_info(
    std::size_t index,
    wchar_t* id,
    std::size_t id_capacity,
    wchar_t* display_name,
    std::size_t display_name_capacity);
extern "C" bool zmk_battery_refresh();
extern "C" bool zmk_battery_device_value(
    std::size_t index,
    wchar_t* value,
    std::size_t value_capacity);
extern "C" bool zmk_battery_tooltip(wchar_t* tooltip, std::size_t tooltip_capacity);

static_assert(sizeof(wchar_t) == 2, "TrafficMonitor requires Windows UTF-16 wchar_t");

class BatteryItem final : public IPluginItem
{
public:
    BatteryItem(std::size_t snapshot_index, std::wstring id, std::wstring display_name)
        : snapshot_index_(snapshot_index),
          item_name_(L"ZMK: " + display_name),
          item_id_(L"zmk_battery_" + std::move(id)),
          label_(item_name_)
    {
    }

    const wchar_t* GetItemName() const override { return item_name_.c_str(); }
    const wchar_t* GetItemId() const override { return item_id_.c_str(); }
    const wchar_t* GetItemLableText() const override { return label_.c_str(); }
    const wchar_t* GetItemValueText() const override { return value_.c_str(); }
    const wchar_t* GetItemValueSampleText() const override { return L"100%*/100%*"; }

    void Refresh()
    {
        std::array<wchar_t, 2048> value{};
        if (zmk_battery_device_value(snapshot_index_, value.data(), value.size()))
        {
            value_ = value.data();
        }
    }

private:
    std::size_t snapshot_index_;
    std::wstring item_name_;
    std::wstring item_id_;
    std::wstring label_;
    std::wstring value_{L"N/A"};
};

class BatteryPlugin final : public ITMPlugin
{
public:
    BatteryPlugin()
    {
        const auto device_count = zmk_battery_device_count();
        items_.reserve(device_count);
        for (std::size_t index = 0; index < device_count; ++index)
        {
            std::array<wchar_t, 2048> id{};
            std::array<wchar_t, 2048> display_name{};
            if (zmk_battery_device_info(
                    index,
                    id.data(),
                    id.size(),
                    display_name.data(),
                    display_name.size()))
            {
                items_.emplace_back(index, id.data(), display_name.data());
            }
        }
    }

    IPluginItem* GetItem(int index) override
    {
        if (index < 0 || static_cast<std::size_t>(index) >= items_.size())
        {
            return nullptr;
        }
        return &items_[static_cast<std::size_t>(index)];
    }

    void DataRequired() override
    {
        if (!zmk_battery_refresh())
        {
            return;
        }

        for (auto& item : items_)
        {
            item.Refresh();
        }

        std::array<wchar_t, 8192> tooltip{};
        if (zmk_battery_tooltip(tooltip.data(), tooltip.size()))
        {
            tooltip_ = tooltip.data();
        }
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
        return tooltip_.c_str();
    }

private:
    std::vector<BatteryItem> items_;
    std::wstring tooltip_{L"Waiting for zmk-battery-center data"};
};

static BatteryPlugin plugin;

extern "C" __declspec(dllexport) ITMPlugin* TMPluginGetInstance()
{
    return &plugin;
}

extern "C" void zmk_tm_link_anchor() {}
