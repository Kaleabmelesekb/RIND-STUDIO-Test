/*
  Rind Studio / Honeydew: a window system for JUCE that has no window system.

  JUCE's Linux windowing talks to X11. Here a ComponentPeer is only a rectangle in memory: the component tree is laid out
  and drawn by JUCE (and, for Vital, by its OpenGL layer, see juce_OpenGL_headless.h) while the embedding app shows the
  rendered frame and feeds the pointer and key events back in through the functions at the bottom of this file.
  Written for this project, GPL-3.0-or-later like the rest of the tree.
*/

namespace juce
{

int juce_headlessDisplayWidth  = 4096;
int juce_headlessDisplayHeight = 4096;

class HeadlessComponentPeer  : public ComponentPeer
{
public:
    HeadlessComponentPeer (Component& comp, int windowStyleFlags)
        : ComponentPeer (comp, windowStyleFlags)
    {
        JUCE_ASSERT_MESSAGE_MANAGER_IS_LOCKED
        getNativeRealtimeModifiers = []() -> ModifierKeys { return ModifierKeys::currentModifiers; };
    }

    ~HeadlessComponentPeer() override = default;

    void* getNativeHandle() const override                       { return (void*) this; }
    void setVisible (bool shouldBeVisible) override              { visible = shouldBeVisible; }
    void setTitle (const String&) override                       {}

    void setBounds (const Rectangle<int>& newBounds, bool isNowFullScreen) override
    {
        bounds = newBounds.withSize (jmax (1, newBounds.getWidth()), jmax (1, newBounds.getHeight()));
        fullScreen = isNowFullScreen;

        WeakReference<Component> deletionChecker (&component);

        if (deletionChecker != nullptr)
            handleMovedOrResized();
    }

    Rectangle<int> getBounds() const override                    { return bounds; }
    BorderSize<int> getFrameSize() const override                { return {}; }

    Point<float> localToGlobal (Point<float> p) override         { return p + bounds.getTopLeft().toFloat(); }
    Point<float> globalToLocal (Point<float> p) override         { return p - bounds.getTopLeft().toFloat(); }
    using ComponentPeer::localToGlobal;
    using ComponentPeer::globalToLocal;

    StringArray getAvailableRenderingEngines() override          { return { "Software Renderer" }; }

    void setMinimised (bool) override                            {}
    bool isMinimised() const override                            { return false; }
    void setFullScreen (bool shouldBeFullScreen) override        { fullScreen = shouldBeFullScreen; }
    bool isFullScreen() const override                           { return fullScreen; }
    void setIcon (const Image&) override                         {}

    bool contains (Point<int> localPos, bool) const override     { return bounds.withZeroOrigin().contains (localPos); }
    bool setAlwaysOnTop (bool) override                          { return true; }
    void toFront (bool makeActive) override                      { if (makeActive) grabFocus(); }
    void toBehind (ComponentPeer*) override                      {}
    bool isFocused() const override                              { return focused; }
    void grabFocus() override                                    { if (! focused) { focused = true; handleFocusGain(); } }
    void textInputRequired (Point<int>, TextInputTarget&) override {}
    void setAlpha (float) override                               {}

    /* The frame is painted by the OpenGL layer; without one these are no-ops (nothing is shown on a window). */
    void repaint (const Rectangle<int>&) override                {}
    void performAnyPendingRepaintsNow() override                 {}

    bool visible = true, fullScreen = false;
    Rectangle<int> bounds;
    bool focused = false;

private:
    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (HeadlessComponentPeer)
};

//==============================================================================
ComponentPeer* Component::createNewPeer (int styleFlags, void*)
{
    return new HeadlessComponentPeer (*this, styleFlags);
}

JUCE_API bool JUCE_CALLTYPE Process::isForegroundProcess()    { return true; }
JUCE_API void JUCE_CALLTYPE Process::makeForegroundProcess()  {}
JUCE_API void JUCE_CALLTYPE Process::hide()                   {}

void Desktop::setKioskComponent (Component* comp, bool enableOrDisable, bool)
{
    if (enableOrDisable)
        comp->setBounds (getDisplays().getDisplayForRect (comp->getScreenBounds())->totalArea);
}

void Displays::findDisplays (float masterScale)
{
    Display d;
    d.isMain = true;
    d.scale = masterScale;
    d.dpi = 96.0;
    d.totalArea = d.userArea = Rectangle<int> (0, 0, juce_headlessDisplayWidth, juce_headlessDisplayHeight);
    displays.add (d);
    updateToLogical();
}

bool Desktop::canUseSemiTransparentWindows() noexcept        { return true; }
void Desktop::setScreenSaverEnabled (bool)                   {}
bool Desktop::isScreenSaverEnabled()                         { return false; }
double Desktop::getDefaultMasterScale()                      { return 1.0; }
Desktop::DisplayOrientation Desktop::getCurrentOrientation() const  { return upright; }
void Desktop::allowedOrientationsChanged()                   {}

//==============================================================================
bool MouseInputSource::SourceList::addSource()
{
    if (sources.isEmpty())
    {
        addSource (0, MouseInputSource::InputSourceType::mouse);
        return true;
    }

    return false;
}

bool MouseInputSource::SourceList::canUseTouch()             { return false; }

static Point<float> juce_headlessMousePosition;

Point<float> MouseInputSource::getCurrentRawMousePosition()  { return juce_headlessMousePosition; }
void MouseInputSource::setRawMousePosition (Point<float> p)  { juce_headlessMousePosition = p; }

void* CustomMouseCursorInfo::create() const                  { return nullptr; }
void MouseCursor::deleteMouseCursor (void*, bool)            {}
void* MouseCursor::createStandardMouseCursor (MouseCursor::StandardCursorType)  { return nullptr; }
void MouseCursor::showInWindow (ComponentPeer*) const        {}

bool DragAndDropContainer::performExternalDragDropOfFiles (const StringArray&, bool, Component*, std::function<void()>)  { return false; }
bool DragAndDropContainer::performExternalDragDropOfText (const String&, Component*, std::function<void()>)              { return false; }

static String juce_headlessClipboard;
void SystemClipboard::copyTextToClipboard (const String& clipText)  { juce_headlessClipboard = clipText; }
String SystemClipboard::getTextFromClipboard()                       { return juce_headlessClipboard; }

bool KeyPress::isKeyCurrentlyDown (int)                      { return false; }
void LookAndFeel::playAlertSound()                           {}

#if JUCE_MODAL_LOOPS_PERMITTED
void JUCE_CALLTYPE NativeMessageBox::showMessageBox (AlertWindow::AlertIconType, const String&, const String&, Component*) {}
#endif

void JUCE_CALLTYPE NativeMessageBox::showMessageBoxAsync (AlertWindow::AlertIconType iconType, const String& title,
                                                          const String& message, Component* associatedComponent,
                                                          ModalComponentManager::Callback* callback)
{
    AlertWindow::showMessageBoxAsync (iconType, title, message, {}, associatedComponent, callback);
}

bool JUCE_CALLTYPE NativeMessageBox::showOkCancelBox (AlertWindow::AlertIconType iconType, const String& title,
                                                      const String& message, Component* associatedComponent,
                                                      ModalComponentManager::Callback* callback)
{
    return AlertWindow::showOkCancelBox (iconType, title, message, {}, {}, associatedComponent, callback);
}

int JUCE_CALLTYPE NativeMessageBox::showYesNoCancelBox (AlertWindow::AlertIconType iconType, const String& title,
                                                        const String& message, Component* associatedComponent,
                                                        ModalComponentManager::Callback* callback)
{
    return AlertWindow::showYesNoCancelBox (iconType, title, message, {}, {}, {}, associatedComponent, callback);
}

int JUCE_CALLTYPE NativeMessageBox::showYesNoBox (AlertWindow::AlertIconType iconType, const String& title,
                                                  const String& message, Component* associatedComponent,
                                                  ModalComponentManager::Callback* callback)
{
    return AlertWindow::showOkCancelBox (iconType, title, message, TRANS ("Yes"), TRANS ("No"), associatedComponent, callback);
}

Image juce_createIconForFile (const File&)                   { return {}; }

//==============================================================================
static const int extendedKeyModifier       = 0x10000;

const int KeyPress::spaceKey        = ' ';
const int KeyPress::returnKey       = 66;
const int KeyPress::escapeKey       = 4;
const int KeyPress::backspaceKey    = 67;
const int KeyPress::leftKey         = extendedKeyModifier + 1;
const int KeyPress::rightKey        = extendedKeyModifier + 2;
const int KeyPress::upKey           = extendedKeyModifier + 3;
const int KeyPress::downKey         = extendedKeyModifier + 4;
const int KeyPress::pageUpKey       = extendedKeyModifier + 5;
const int KeyPress::pageDownKey     = extendedKeyModifier + 6;
const int KeyPress::endKey          = extendedKeyModifier + 7;
const int KeyPress::homeKey         = extendedKeyModifier + 8;
const int KeyPress::deleteKey       = extendedKeyModifier + 9;
const int KeyPress::insertKey       = -1;
const int KeyPress::tabKey          = 61;
const int KeyPress::F1Key           = extendedKeyModifier + 10;
const int KeyPress::F2Key           = extendedKeyModifier + 11;
const int KeyPress::F3Key           = extendedKeyModifier + 12;
const int KeyPress::F4Key           = extendedKeyModifier + 13;
const int KeyPress::F5Key           = extendedKeyModifier + 14;
const int KeyPress::F6Key           = extendedKeyModifier + 16;
const int KeyPress::F7Key           = extendedKeyModifier + 17;
const int KeyPress::F8Key           = extendedKeyModifier + 18;
const int KeyPress::F9Key           = extendedKeyModifier + 19;
const int KeyPress::F10Key          = extendedKeyModifier + 20;
const int KeyPress::F11Key          = extendedKeyModifier + 21;
const int KeyPress::F12Key          = extendedKeyModifier + 22;
const int KeyPress::F13Key          = extendedKeyModifier + 23;
const int KeyPress::F14Key          = extendedKeyModifier + 24;
const int KeyPress::F15Key          = extendedKeyModifier + 25;
const int KeyPress::F16Key          = extendedKeyModifier + 26;
const int KeyPress::F17Key          = extendedKeyModifier + 50;
const int KeyPress::F18Key          = extendedKeyModifier + 51;
const int KeyPress::F19Key          = extendedKeyModifier + 52;
const int KeyPress::F20Key          = extendedKeyModifier + 53;
const int KeyPress::F21Key          = extendedKeyModifier + 54;
const int KeyPress::F22Key          = extendedKeyModifier + 55;
const int KeyPress::F23Key          = extendedKeyModifier + 56;
const int KeyPress::F24Key          = extendedKeyModifier + 57;
const int KeyPress::F25Key          = extendedKeyModifier + 58;
const int KeyPress::F26Key          = extendedKeyModifier + 59;
const int KeyPress::F27Key          = extendedKeyModifier + 60;
const int KeyPress::F28Key          = extendedKeyModifier + 61;
const int KeyPress::F29Key          = extendedKeyModifier + 62;
const int KeyPress::F30Key          = extendedKeyModifier + 63;
const int KeyPress::F31Key          = extendedKeyModifier + 64;
const int KeyPress::F32Key          = extendedKeyModifier + 65;
const int KeyPress::F33Key          = extendedKeyModifier + 66;
const int KeyPress::F34Key          = extendedKeyModifier + 67;
const int KeyPress::F35Key          = extendedKeyModifier + 68;
const int KeyPress::numberPad0      = extendedKeyModifier + 27;
const int KeyPress::numberPad1      = extendedKeyModifier + 28;
const int KeyPress::numberPad2      = extendedKeyModifier + 29;
const int KeyPress::numberPad3      = extendedKeyModifier + 30;
const int KeyPress::numberPad4      = extendedKeyModifier + 31;
const int KeyPress::numberPad5      = extendedKeyModifier + 32;
const int KeyPress::numberPad6      = extendedKeyModifier + 33;
const int KeyPress::numberPad7      = extendedKeyModifier + 34;
const int KeyPress::numberPad8      = extendedKeyModifier + 35;
const int KeyPress::numberPad9      = extendedKeyModifier + 36;
const int KeyPress::numberPadAdd            = extendedKeyModifier + 37;
const int KeyPress::numberPadSubtract       = extendedKeyModifier + 38;
const int KeyPress::numberPadMultiply       = extendedKeyModifier + 39;
const int KeyPress::numberPadDivide         = extendedKeyModifier + 40;
const int KeyPress::numberPadSeparator      = extendedKeyModifier + 41;
const int KeyPress::numberPadDecimalPoint   = extendedKeyModifier + 42;
const int KeyPress::numberPadEquals         = extendedKeyModifier + 43;
const int KeyPress::numberPadDelete         = extendedKeyModifier + 44;
const int KeyPress::playKey         = extendedKeyModifier + 45;
const int KeyPress::stopKey         = extendedKeyModifier + 46;
const int KeyPress::fastForwardKey  = extendedKeyModifier + 47;
const int KeyPress::rewindKey       = extendedKeyModifier + 48;

bool juce_areThereAnyAlwaysOnTopWindows() { return false; }

/*  What the embedding app calls (all on the message thread). */

void juce_headlessPointer (Component& root, int type, float x, float y, int modifierFlags, float pressure, int64 timeMs)
{
    // type: 0 move, 1 down, 2 up (the buttons are carried by the modifier flags, as JUCE keeps them).
    juce_headlessMousePosition = { x, y };
    auto mods = ModifierKeys (modifierFlags);
    ModifierKeys::currentModifiers = mods;

    if (auto* peer = root.getPeer())
        peer->handleMouseEvent (MouseInputSource::InputSourceType::mouse, { x, y }, mods, pressure, 0.0f, timeMs);
}

void juce_headlessWheel (Component& root, float x, float y, float dx, float dy, int modifierFlags, int64 timeMs)
{
    ModifierKeys::currentModifiers = ModifierKeys (modifierFlags);

    if (auto* peer = root.getPeer())
    {
        MouseWheelDetails w;
        w.deltaX = dx;
        w.deltaY = dy;
        w.isReversed = false;
        w.isSmooth = true;
        w.isInertial = false;
        peer->handleMouseWheel (MouseInputSource::InputSourceType::mouse, { x, y }, timeMs, w);
    }
}

} // namespace juce
