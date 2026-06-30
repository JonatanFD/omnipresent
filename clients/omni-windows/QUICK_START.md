# Quick Start: Nueva estructura de Windows GUI

## 📂 Dónde encontrar qué

### Para entender la arquitectura:
- **ARCHITECTURE.md** - Diagramas ASCII y flujos de datos
- **REFACTOR_SUMMARY.md** - Qué cambió, por qué, y próximas fases
- **ANTES_DESPUES.md** - Comparativa visual antes/después
- **Este archivo** - Guía rápida para desarrolladores

### Para trabajar con las vistas:

**Daemon control:**
```
Views/GeneralView.xaml          ← Status, start/stop daemon, info local
```

**Conexiones y peers:**
```
Views/ConnectionsView.xaml      ← Connect, incoming requests, sessions, peers, layout
```

**Sistema:**
```
Views/SystemView.xaml           ← Clipboard toggle (expandible)
```

**Actualizaciones:**
```
Views/UpdateView.xaml           ← Check version, descargar actualizaciones
```

**Navegación:**
```
MainView.xaml                   ← Contenedor (no tocar mucho)
MainView.xaml.cs                ← Lógica de navegación
```

**Binding global:**
```
App.xaml                        ← Converters registrados
Converters/ValueConverters.cs   ← BoolToVisibility, etc.
```

## 🔧 Tareas comunes

### Cambiar el contenido de una sección
1. Abre `Views/[Section]View.xaml`
2. Modifica el XAML como necesites
3. Si necesitas lógica, ve a `[Section]View.xaml.cs`

**Ejemplo:** Agregar un nuevo toggle en SystemView
```xaml
<!-- Views/SystemView.xaml -->
<ToggleSwitch Header="My new setting"
              IsOn="{x:Bind ViewModel.MyNewProperty, Mode=OneWay}"
              IsEnabled="{x:Bind ViewModel.IsConnected, Mode=OneWay}"
              Toggled="OnMyNewToggled" />
```

```csharp
// Views/SystemView.xaml.cs
private async void OnMyNewToggled(object sender, RoutedEventArgs e)
{
    var toggle = (ToggleSwitch)sender;
    if (toggle.IsOn != ViewModel.MyNewProperty)
    {
        await ViewModel.SetMyNewPropertyAsync(toggle.IsOn);
    }
}
```

### Agregar una nueva sección completa
1. Copiar `Views/SystemView.xaml` → `Views/NewFeatureView.xaml`
2. Copiar `Views/SystemView.xaml.cs` → `Views/NewFeatureView.xaml.cs`
3. Renombrar class a `NewFeatureView`
4. Agregar item a NavigationView en `MainView.xaml`:
```xaml
<NavigationViewItem Content="New Feature" Icon="Home" Tag="newfeature" />
```
5. Agregar case en `MainView.xaml.cs`:
```csharp
private void NavigateToSection(string section)
{
    var view = section switch
    {
        "general" => new GeneralView(ViewModel),
        "connections" => new ConnectionsView(ViewModel),
        "system" => new SystemView(ViewModel),
        "update" => new UpdateView(ViewModel),
        "newfeature" => new NewFeatureView(ViewModel),  // ← NUEVO
        _ => new GeneralView(ViewModel),
    };
    ContentFrame.Content = view;
}
```

### Usar un converter
```xaml
<!-- Registrados en App.xaml, usables directamente -->
<StackPanel Visibility="{x:Bind ViewModel.IsConnected, Mode=OneWay, Converter={StaticResource BoolToVisibility}}">
```

### Crear un nuevo converter
1. Agregar clase a `Converters/ValueConverters.cs`:
```csharp
public sealed class MyCustomConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        // Tu lógica aquí
        return result;
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
    {
        throw new NotImplementedException();
    }
}
```
2. Registrar en `App.xaml`:
```xaml
<converters:MyCustomConverter x:Key="MyCustomConverter" />
```
3. Usar en cualquier XAML:
```xaml
<TextBlock Text="{x:Bind ViewModel.Value, Converter={StaticResource MyCustomConverter}}" />
```

## 🎯 Estructura mental

Piensa en cada vista como **una página de un sitio web**:
- Tiene su propia responsabilidad
- Renderiza desde el ViewModel
- Llama métodos en el ViewModel cuando el usuario interactúa
- No toca directamente a other vistas

```
User interacts
    ↓
View click handler (xaml.cs)
    ↓
ViewModel method call (async)
    ↓
ViewModel updates properties
    ↓
All views using those properties auto-update
```

## 📚 Patrón de binding en cada vista

Todas las vistas siguen este patrón:

```csharp
// Archivo.xaml.cs
public sealed partial class MyView : UserControl
{
    public DaemonViewModel ViewModel { get; }

    public MyView(DaemonViewModel viewModel)
    {
        ViewModel = viewModel;
        InitializeComponent();
    }

    // Event handlers para botones/toggles
    private async void OnMyButtonClick(object sender, RoutedEventArgs e)
    {
        await ViewModel.SomeCommandAsync(args);
    }
}
```

```xaml
<!-- Archivo.xaml -->
<UserControl>
    <!-- Binding al ViewModel -->
    <TextBlock Text="{x:Bind ViewModel.Property, Mode=OneWay}" />
    
    <!-- Event handler -->
    <Button Click="OnMyButtonClick" Content="Click me" />
    
    <!-- Colecciones (automáticas) -->
    <ItemsControl ItemsSource="{x:Bind ViewModel.Items, Mode=OneWay}">
        <ItemsControl.ItemTemplate>
            <DataTemplate x:DataType="ipc:ItemInfo">
                <!-- Renderear cada item -->
            </DataTemplate>
        </ItemsControl.ItemTemplate>
    </ItemsControl>
</UserControl>
```

## 🛠️ Compilar y ejecutar

```bash
# Build
cd clients/omni-windows
dotnet build

# Ejecutar
dotnet run --project src/Omni.App

# O directo desde VS: F5
```

## 🐛 Debug

1. **Ver qué view está activa:**
   - Mira el NavigationView.SelectedItem en MainView
   - La vista activa está en ContentFrame.Content

2. **Binding no funciona:**
   - Asegúrate de que ViewModel tiene `public Task MyAsync()`
   - Usa `await ViewModel.MyAsync()` en el click handler
   - El ViewModel debe ser ObservableObject

3. **Los datos no se actualizan:**
   - Verifica que las propiedades usen `SetField()`
   - Las colecciones deben ser ObservableCollection<T>
   - Usa Mode=OneWay en los bindings

## 📖 Referencias

- **ARCHITECTURE.md** - Para entender el flujo de datos
- **Código de macOS** - En `clients/omni-macos/` para ver cómo hacen cosas similares
- **DaemonViewModel.cs** - Todas las propiedades y comandos disponibles

## ✨ Tips pro

1. **Hot reload durante debug:**
   - En Visual Studio: Edit & Continue funciona
   - Los bindings se actualizan automáticamente

2. **Reutilizar DataTemplates:**
   - Próxima fase: mover a Components/ para evitar duplicación
   - Ahora mismo cada vista tiene sus templates (ok para Phase 1)

3. **Errores de binding:**
   - Visual Studio Output window muestra errores de binding
   - Si ves "Binding creation failed", revisa el console output

4. **Navigate sin NavigationView:**
   - Si necesitas ir a otra vista desde código:
   - `MainView` no es accesible (no hace falta, usar la navegación del UI)

## 🎓 Aprendizaje progresivo

1. **Día 1:** Lee ARCHITECTURE.md
2. **Día 2:** Modifica un simple toggle en SystemView
3. **Día 3:** Agrega una nueva vista siguiendo el patrón
4. **Día 4:** Crea un nuevo converter
5. **Día 5:** Entiende el flujo IPC de end-to-end

¡Diviértete! La arquitectura está limpia y es fácil trabajar. 🚀
