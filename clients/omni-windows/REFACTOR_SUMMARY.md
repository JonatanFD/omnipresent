# Windows GUI Refactor: Modular Architecture

## 📋 Resumen de cambios

Se ha refactorizado completamente la GUI de Windows para seguir el patrón modular de macOS.

### Estructura anterior (Monolítica)
```
MainView.xaml (196 líneas)
└── Todo el contenido en una sola vista
```

### Estructura nueva (Modular)
```
Omni.App/
├── MainView.xaml                # Contenedor de navegación (frame)
├── MainView.xaml.cs              # Lógica de navegación
├── App.xaml / App.xaml.cs        # Converters registrados
├── Views/
│   ├── GeneralView.xaml/cs       # Status, start/stop daemon, info
│   ├── ConnectionsView.xaml/cs   # Connect, pending, sessions, peers, layout
│   ├── SystemView.xaml/cs        # Clipboard toggle
│   ├── UpdateView.xaml/cs        # Version info, update button
├── Converters/
│   └── ValueConverters.cs        # BoolToVisibility, BoolNegation, EmptyStringToVisibility
└── App.Core/
    └── DaemonViewModel.cs        # +StartDaemonAsync, StopDaemonAsync, ReconnectNow
```

## 🎯 Cambios principales

### 1. **MainView.xaml** → Contenedor de navegación
- **Antes:** ScrollViewer + StackPanel monolítico (196 líneas)
- **Ahora:** NavigationView + Frame para cambiar vistas (30 líneas)
- **Beneficio:** Separación clara de secciones, fácil de navegar

```xaml
<NavigationView>
  <NavigationViewItem Content="General" Tag="general" />
  <NavigationViewItem Content="Connections" Tag="connections" />
  <NavigationViewItem Content="System" Tag="system" />
  <NavigationViewItem Content="Update" Tag="update" />
  <Frame x:Name="ContentFrame" />
</NavigationView>
```

### 2. **GeneralView** - Status y control del daemon
- Muestra estado de conexión (indicador visual)
- Botones Start/Stop para el daemon
- Info: fingerprint, puerto, input capture, versión
- Manejo de incompatibilidad con el daemon

### 3. **ConnectionsView** - Gestión de conexiones
- Connect a peers
- Aceptar/rechazar requests entrantes (TOFU)
- Listar sesiones activas
- Listar peers conocidos
- Configurar layout (posición de pantallas)
- Empty state cuando daemon está apagado

### 4. **SystemView** - Configuración del sistema
- Toggle para compartir clipboard
- Expandible para futuras opciones de sistema

### 5. **UpdateView** - Actualizaciones
- Mostrar versión instalada
- Botón para descargar e instalar actualizaciones
- Feedback visual durante la actualización

### 6. **Converters** - Value converters globales
- `BoolToVisibilityConverter` - Mostrar/ocultar basado en bool
- `BoolNegationConverter` - Invertir valor bool
- `EmptyStringToVisibilityConverter` - Mostrar solo si hay contenido

### 7. **DaemonViewModel** - Métodos nuevos
```csharp
public async Task StartDaemonAsync()      // Inicia el daemon
public Task StopDaemonAsync()             // Detiene el daemon
public void ReconnectNow()                // Reconecta inmediatamente
```

## 🔄 Comparativa con macOS

| Aspecto | macOS | Windows Antes | Windows Ahora |
|---------|-------|---|---|
| **Vistas** | 5 (General, Connections, System, Update, MenuBar) | 1 | 4 (+MenuBar futuro) |
| **Navegación** | Sidebar + Detail pane | Scroll vertical | NavigationView + Frame |
| **Modularidad** | ✅ Alta | ❌ Nula | ✅ Alta |
| **Componentes reutilizables** | ✅ Sí | ❌ No | ✅ Sí |
| **Escalabilidad** | ✅ Fácil | ❌ Difícil | ✅ Fácil |

## 📦 Beneficios

1. **Mantenibilidad**: Cada sección en un archivo separado
2. **Escalabilidad**: Fácil agregar nuevas vistas/funcionalidades
3. **Testing**: Cada vista puede testearse independientemente
4. **Coherencia**: Mismo patrón que macOS
5. **Performance**: Vistas se cargan bajo demanda
6. **UX**: Navegación clara entre secciones

## 🚀 Próximos pasos

### Fase 2: Componentes reutilizables
- [ ] Crear `PeerCard.xaml` - Card genérico para peer/sesión
- [ ] Crear `PendingRequestCard.xaml` - Card para requests
- [ ] Refactorizar ConnectionsView para usar estos componentes

### Fase 3: Sistema de tray (opcional)
- [ ] Implementar AppBar en Windows 11+
- [ ] Mostrar pending requests en tray
- [ ] Notificaciones del sistema

### Fase 4: Polish visual
- [ ] Animaciones en navegación
- [ ] Transiciones suaves entre vistas
- [ ] Iconos consistentes
- [ ] Respuesta a cambios de tema

### Fase 5: Dark mode
- [ ] Pruebas exhaustivas con light/dark theme
- [ ] Ajustes de contraste si es necesario

## ⚙️ Compilación

```bash
cd clients/omni-windows
dotnet build
dotnet run
```

## ⚠️ Notas técnicas

1. **NavigationView** vs **TabView**: 
   - NavigationView es mejor para aplicaciones con muchas secciones
   - Es el estándar en WinUI 3 para navegación lateral

2. **Frame Navigation**:
   - Cada click en el menú navega a una vista diferente
   - El Frame mantiene el stack de navegación (Back button en futuro)

3. **DaemonViewModel**:
   - Es ObservableObject, así que los cambios se propagan automáticamente a las vistas
   - Las vistas se suscriben a PropertyChanged para updates dinámicos

4. **Converters**:
   - Registrados globalmente en App.xaml
   - Pueden usarse en cualquier XAML sin redefinir

## 📝 Notas para el futuro

- Considerar agregar animaciones de transición entre vistas
- Implementar back button si se necesita navegación más compleja
- Agregar breadcrumb para orientación del usuario
- Considerar agregar vista de settings/preferences
