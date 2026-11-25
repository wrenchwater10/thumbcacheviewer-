/*
	thumbcache_viewer will extract thumbnail images from thumbcache database files.
	Copyright (C) 2011-2023 Eric Kutcher

	This program is free software: you can redistribute it and/or modify
	it under the terms of the GNU General Public License as published by
	the Free Software Foundation, either version 3 of the License, or
	(at your option) any later version.

	This program is distributed in the hope that it will be useful,
	but WITHOUT ANY WARRANTY; without even the implied warranty of
	MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
	GNU General Public License for more details.

	You should have received a copy of the GNU General Public License
	along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

#include "globals.h"
#include "utilities.h"
#include "menus.h"

#include "lite_user32.h"

#include <stdio.h>

#define IDT_TIMER		2001

#define MIN_SCALE_SIZE	0.1f
#define MAX_SCALE_SIZE	25.0f

// Window variables
int cx_i = 0;				// Current x (left) position of the image window based on the mouse.
int cy_i = 0;				// Current y (top) position of the image window based on the mouse.

bool mouse_r_down = false;	// Toggled when the right mouse button is down.
bool mouse_l_down = false;	// Toggled when the right mouse button is down.
bool rb_zoomed = false;		// Toggled when the current right button press performed a zoom.

// Image variables
PAINTSTRUCT ps = { NULL };	// The paint structure used to draw on the image window.
float scale = 1.0f;			// Scale of the image.
float zoom_step = 0.5f;		// Increment used when zooming in and out. Adjustable in Tools -> Settings.

POINT drag_rect = { 0 };	// The current position of gdi_image in the image window.
POINT old_pos = { 0 };		// The old position of gdi_image. Used to calculate the rate of change.

// Timer variables.
bool zoom = false;			// Toggled when we want to activate the timer and display the zoom text.
bool timer_active = false;	// Toggled when the timer is active and used to reset it.

UINT current_dpi_image = 0;//, last_dpi_image = USER_DEFAULT_SCREEN_DPI;

HFONT hFont_image = NULL;

LRESULT CALLBACK ImageWndProc( HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam )
{
	switch ( msg )
	{
		case WM_CREATE:
		{
			current_dpi_image = GetDpiForWindow( hWnd );
			hFont_image = UpdateFontsAndMetrics( current_dpi_image, /*last_dpi_image,*/ NULL );

			return 0;
		}
		break;

		case WM_KEYDOWN:
		{
			switch ( wParam )
			{
				case VK_HOME:
				{
					RECT rc;
					GetClientRect( hWnd, &rc );

					// Center the image.
					drag_rect.x = ( ( long )gdi_image->GetWidth() - rc.right ) / 2;
					drag_rect.y = ( ( long )gdi_image->GetHeight() - rc.bottom ) / 2;

					// Redraw our image.
					InvalidateRect( hWnd, NULL, TRUE );
				}
				break;

				case VK_OEM_PLUS:
				{
					// Zoom in.
					scale += zoom_step;
					if ( scale < MIN_SCALE_SIZE )
					{
						scale = MIN_SCALE_SIZE;
					}
					else if ( scale > MAX_SCALE_SIZE )
					{
						scale = MAX_SCALE_SIZE;
					}

					timer_active = false;	// Allow the timer to reset.
					zoom = true;

					// Redraw our image.
					InvalidateRect( hWnd, NULL, TRUE );
					
				}
				break;

				case VK_OEM_MINUS:
				{
					// Zoom out.
					scale -= zoom_step;
					if ( scale < MIN_SCALE_SIZE )
					{
						scale = MIN_SCALE_SIZE;
					}
					else if ( scale > MAX_SCALE_SIZE )
					{
						scale = MAX_SCALE_SIZE;
					}

					timer_active = false;	// Allow the timer to reset.
					zoom = true;

					// Redraw our image.
					InvalidateRect( hWnd, NULL, TRUE );
				}
				break;

				case VK_LEFT:
				{
					// See if the control or shift key is down.
					if ( GetKeyState( VK_SHIFT ) & 0x8000 )
					{
						drag_rect.x += 25;
					}
					else if ( GetKeyState( VK_CONTROL ) & 0x8000 )
					{
						drag_rect.x += 5;
					}
					else
					{
						drag_rect.x += 1;
					}

					// Prevent the image from going more than its width offscreen.
					float left = ( gdi_image->GetWidth() * scale ) - ( ( gdi_image->GetWidth() / 2 ) * ( scale - 1 ) );
					if ( drag_rect.x >= left )
					{
						drag_rect.x = ( long )left;
					}

					// Redraw our image.
					InvalidateRect( hWnd, NULL, TRUE );
				}
				break;

				case VK_RIGHT:
				{
					RECT rc;
					GetClientRect( hWnd, &rc );

					// See if the control or shift key is down.
					if ( GetKeyState( VK_SHIFT ) & 0x8000 )
					{
						drag_rect.x -= 25;
					}
					else if ( GetKeyState( VK_CONTROL ) & 0x8000 )
					{
						drag_rect.x -= 5;
					}
					else
					{
						drag_rect.x -= 1;
					}

					// Prevent the image from going more than its width offscreen.
					float right = rc.right + ( ( gdi_image->GetWidth() / 2 ) * ( scale - 1 ) );
					if ( -drag_rect.x >=  right )
					{
						drag_rect.x = ( long )-right;
					}

					// Redraw our image.
					InvalidateRect( hWnd, NULL, TRUE );
				}
				break;

				case VK_UP:
				{
					// See if the control or shift key is down.
					if ( GetKeyState( VK_SHIFT ) & 0x8000 )
					{
						drag_rect.y += 25;
					}
					else if ( GetKeyState( VK_CONTROL ) & 0x8000 )
					{
						drag_rect.y += 5;
					}
					else
					{
						drag_rect.y += 1;
					}

					// Prevent the image from going more than its height offscreen.
					float top = ( gdi_image->GetHeight() * scale ) - ( ( gdi_image->GetHeight() / 2 ) * ( scale - 1 ) );
					if ( drag_rect.y >= top )
					{
						drag_rect.y = ( long )top;
					}

					// Redraw our image.
					InvalidateRect( hWnd, NULL, TRUE );
				}
				break;

				case VK_DOWN:
				{
					RECT rc;
					GetClientRect( hWnd, &rc );
				
					// See if the control or shift key is down.
					if ( GetKeyState( VK_SHIFT ) & 0x8000 )
					{
						drag_rect.y -= 25;
					}
					else if ( GetKeyState( VK_CONTROL ) & 0x8000 )
					{
						drag_rect.y -= 5;
					}
					else
					{
						drag_rect.y -= 1;
					}

					// Prevent the image from going more than its height offscreen.
					float bottom = rc.bottom + ( ( gdi_image->GetHeight() / 2 ) * ( scale - 1 ) );
					if ( -drag_rect.y >= bottom )
					{
						drag_rect.y = ( long )-bottom;
					}

					// Redraw our image.
					InvalidateRect( hWnd, NULL, TRUE );
				}
				break;
			}
			return 0;
		}
		break;

		case WM_MOUSEWHEEL:
		{
			// Increment, or decrement our scale based on the scrollwheel change.
			scale += zoom_step * ( GET_WHEEL_DELTA_WPARAM( wParam ) / WHEEL_DELTA );
			if ( scale < MIN_SCALE_SIZE )
			{
				scale = MIN_SCALE_SIZE;
			}
			else if ( scale > MAX_SCALE_SIZE )
			{
				scale = MAX_SCALE_SIZE;
			}

			timer_active = false;	// Allow the timer to reset.
			zoom = true;

			// Redraw our image.
			InvalidateRect( hWnd, NULL, TRUE );

			return 0;
		}
		break;

		case WM_MOUSEMOVE:
		{
			if ( wParam == MK_LBUTTON )
			{
				// Set our image position based on the rate of change between mouse movements.
				drag_rect.x += old_pos.x - LOWORD( lParam );
				drag_rect.y += old_pos.y - HIWORD( lParam );

				// This will make sure that the image can't go offscreen by more pixels than its width.
				float left = ( gdi_image->GetWidth() * scale ) - ( ( gdi_image->GetWidth() / 2 ) * ( scale - 1 ) );
				if ( drag_rect.x >= left )
				{
					drag_rect.x = ( long )left;
				}

				// This will make sure that the image can't go offscreen by more pixels than its height.
				float top = ( gdi_image->GetHeight() * scale ) - ( ( gdi_image->GetHeight() / 2 ) * ( scale - 1 ) );
				if ( drag_rect.y >= top )
				{
					drag_rect.y = ( long )top;
				}

				RECT rc;
				GetClientRect( hWnd, &rc );
				// This will make sure that the image can't go offscreen by more pixels than its width.
				float right = rc.right + ( ( gdi_image->GetWidth() / 2 ) * ( scale - 1 ) );
				if ( -drag_rect.x >=  right )
				{
					drag_rect.x = ( long )-right;
				}

				// This will make sure that the image can't go offscreen by more pixels than its height.
				float bottom = rc.bottom + ( ( gdi_image->GetHeight() / 2 ) * ( scale - 1 ) );
				if ( -drag_rect.y >= bottom )
				{
					drag_rect.y = ( long )-bottom;
				}

				// Redraw our image.
				InvalidateRect( hWnd, NULL, TRUE );

				// Set our old mouse position.
				old_pos.x = LOWORD( lParam );
				old_pos.y = HIWORD( lParam );
			}
			return 0;
		}
		break;

		case WM_LBUTTONDOWN:
		{
			// Take the focus so the + and - zoom keys work while the preview pane is active.
			SetFocus( hWnd );

			mouse_l_down = true;

			// See if the right mouse button is down.
			if ( mouse_r_down )
			{
				// Zoom in.
				scale += zoom_step;
				if ( scale < MIN_SCALE_SIZE )
				{
					scale = MIN_SCALE_SIZE;
				}
				else if ( scale > MAX_SCALE_SIZE )
				{
					scale = MAX_SCALE_SIZE;
				}

				timer_active = false;	// Allow the timer to reset.
				zoom = true;
				rb_zoomed = true;

				// Redraw our image.
				InvalidateRect( hWnd, NULL, TRUE );
			}
			else
			{
				// Save our initial mouse down position.
				old_pos.x = LOWORD( lParam );
				old_pos.y = HIWORD( lParam );
			}

			return 0;
		}
		break;

		case WM_LBUTTONUP:
		{
			mouse_l_down = false;
			return 0;
		}
		break;

		case WM_RBUTTONDOWN:
		{
			// Take the focus so the + and - zoom keys work while the preview pane is active.
			SetFocus( hWnd );

			mouse_r_down = true;
			rb_zoomed = false;

			// See if the left mouse button is down.
			if ( mouse_l_down )
			{
				// Zoom out.
				scale -= zoom_step;
				if ( scale < MIN_SCALE_SIZE )
				{
					scale = MIN_SCALE_SIZE;
				}
				else if ( scale > MAX_SCALE_SIZE )
				{
					scale = MAX_SCALE_SIZE;
				}

				timer_active = false;	// Allow the timer to reset.
				zoom = true;
				rb_zoomed = true;

				// Redraw our image.
				InvalidateRect( hWnd, NULL, TRUE );
			}
			return 0;
		}
		break;

		case WM_RBUTTONUP:
		{
			mouse_r_down = false;

			// Show the context menu, unless this right click was part of a zoom gesture.
			if ( !rb_zoomed )
			{
				POINT p;
				GetCursorPos( &p );

				UpdateMenus( UM_ENABLE );

				TrackPopupMenu( g_hMenuSub_context, TPM_RIGHTBUTTON, p.x, p.y, 0, hWnd, NULL );
			}

			rb_zoomed = false;

			return 0;
		}
		break;

		case WM_COMMAND:
		{
			// Context menu commands are handled by the main window.
			SendMessage( g_hWnd_main, WM_COMMAND, wParam, lParam );
			return 0;
		}
		break;

		case WM_MBUTTONDOWN:
		{
			RECT rc;
			GetClientRect( hWnd, &rc );

			// Center the image.
			drag_rect.x = ( ( long )gdi_image->GetWidth() - rc.right ) / 2;
			drag_rect.y = ( ( long )gdi_image->GetHeight() - rc.bottom ) / 2;

			// Force our window to repaint itself.
			InvalidateRect( hWnd, NULL, TRUE );
			return 0;
		}
		break;

		case WM_ERASEBKGND:
		{
			// We'll handle the background drawing.
			return TRUE;
		}
		break;

		case WM_PAINT:
		{
			HDC hDC = BeginPaint( hWnd, &ps );

			RECT rc;
			GetClientRect( hWnd, &rc );

			// Make sure we have an image to draw.
			if ( gdi_image != NULL )
			{

				// Create a memory buffer to draw to.
				HDC hdcMem = CreateCompatibleDC( hDC );

				// Create a bitmap in our memory buffer that's the size of our window.
				HBITMAP hbm = CreateCompatibleBitmap( hDC, rc.right, rc.bottom );
				HBITMAP ohbm = ( HBITMAP )SelectObject( hdcMem, hbm );
				DeleteObject( ohbm );
				DeleteObject( hbm );

				// Fill the memory background with the menu color
				HBRUSH background = CreateSolidBrush( ( COLORREF )GetSysColor( COLOR_MENU ) );
				FillRect( hdcMem, &rc, background );
				DeleteObject( background );

				// Create a graphics object for our memory buffer.
				Gdiplus::Graphics graphics( hdcMem );
					
				// Draw the image on screen.
				if ( scale != 1.0f || current_dpi_image != USER_DEFAULT_SCREEN_DPI )
				{
					float scale2 = scale * current_dpi_image / USER_DEFAULT_SCREEN_DPI;

					// Scale the image. Use a smoothing filter when the image is being shrunk.
					graphics.SetInterpolationMode( ( scale2 < 1.0f ) ? Gdiplus::InterpolationModeHighQualityBicubic : Gdiplus::InterpolationModeNearestNeighbor );
					graphics.ScaleTransform( scale2, scale2 );
					// Whoa! This draws the image around its scaled center.
					graphics.DrawImage( gdi_image, ( ( -drag_rect.x - ( ( gdi_image->GetWidth() / 2 ) * ( scale2 - 1 ) ) ) / scale2 ), ( ( -drag_rect.y - ( ( gdi_image->GetHeight() / 2 ) * ( scale2 - 1 ) ) ) / scale2 ) );
				}
				else
				{
					graphics.DrawImage( gdi_image, ( INT )-drag_rect.x, ( INT )-drag_rect.y );
				}

				// If the image has just been zoomed, display some text about it.
				if ( zoom )
				{
					// Prevents InvalidateRect calls from interfering with an active timer.
					if ( !timer_active )
					{
						timer_active = true;
						SetTimer( hWnd, IDT_TIMER, 5000, ( TIMERPROC )TimerProc );
					}

					char buf[ 32 ] = { 0 };
					sprintf_s( buf, 32, " Zoom level: %4.2fx ", scale );

					HFONT ohf = ( HFONT )SelectObject( hdcMem, hFont_image );
					DeleteObject( ohf );

					SetBkColor( hdcMem, ( COLORREF )GetSysColor( COLOR_MENU ) );
					SetTextColor( hdcMem, RGB( 0x00, 0x00, 0x00 ) );
					DrawTextA( hdcMem, buf, -1, &rc, DT_SINGLELINE );
				}

				// Draw our memory buffer to the main device context.
				BitBlt( hDC, 0, 0, rc.right, rc.bottom, hdcMem, 0, 0, SRCCOPY );
				DeleteDC( hdcMem );
			}
			else
			{
				// Nothing is selected. Fill the preview pane with our background color.
				HBRUSH background = CreateSolidBrush( ( COLORREF )GetSysColor( COLOR_MENU ) );
				FillRect( hDC, &rc, background );
				DeleteObject( background );
			}

			EndPaint( hWnd, &ps );

			return 0;
		}
		break;

		/*case WM_GETMINMAXINFO:
		{
			// Set the minimum dimensions that the window can be sized to.
			( ( MINMAXINFO * )lParam )->ptMinTrackSize.x = MIN_HEIGHT / 2;
			( ( MINMAXINFO * )lParam )->ptMinTrackSize.y = MIN_HEIGHT / 2;

			return 0;
		}
		break;*/

		case WM_DPICHANGED:
		{
			//last_dpi_image = current_dpi_image;
			current_dpi_image = HIWORD( wParam );
			HFONT hFont = UpdateFontsAndMetrics( current_dpi_image, /*last_dpi_image,*/ NULL );
			EnumChildWindows( hWnd, EnumChildProc, ( LPARAM )hFont );
			DeleteObject( hFont_image );
			hFont_image = hFont;

			return 0;
		}
		break;

		case WM_DESTROY:
		{
			// Delete our font.
			DeleteObject( hFont_image );

			return 0;
		}
		break;

		default:
		{
			return DefWindowProc( hWnd, msg, wParam, lParam );
		}
		break;
	}
}

VOID CALLBACK TimerProc( HWND hWnd, UINT /*msg*/, UINT /*idTimer*/, DWORD /*dwTime*/ )
{
	zoom = false;

	// Redraw our image.
	InvalidateRect( hWnd, NULL, TRUE );

	// Stop our timer.
	timer_active = false;
	KillTimer( hWnd, IDT_TIMER );
}
